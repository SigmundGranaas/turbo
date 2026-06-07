import Foundation
import CoreCommon
import AuthenticationServices

/// Google sign-in via the Turbo API's OAuth redirect — mirrors the Android flow
/// (`AuthRepository.loginWithGoogle`): open the API's Google auth URL in an
/// `ASWebAuthenticationSession`, capture the `turbo://oauth?code=…` callback,
/// then exchange the code with the API for an account + tokens.
///
/// The two pure steps — pulling the `code` out of the callback URL and decoding
/// the API's auth response — are factored out so they're unit-testable without a
/// browser or network.
public final class GoogleAuthRepository: NSObject, AuthRepository, @unchecked Sendable {
    private let store: ReactiveStore<AuthState>
    private let tokenStore: ReactiveStore<String?>
    private let apiBaseURL: URL
    private let callbackScheme: String
    private nonisolated(unsafe) let session: URLSession

    public init(apiBaseURL: URL, callbackScheme: String = "turbo", session: URLSession = .shared) {
        self.store = ReactiveStore(.signedOut)
        self.tokenStore = ReactiveStore(nil)
        self.apiBaseURL = apiBaseURL
        self.callbackScheme = callbackScheme
        self.session = session
    }

    public func state() async -> AsyncStream<AuthState> { await store.stream() }
    public func current() async -> AuthState { await store.current() }
    public func token() async -> String? { await tokenStore.current() }

    public func signOut() async {
        await store.set(.signedOut)
        await tokenStore.set(nil)
    }

    public func signIn() async -> Outcome<Account> {
        do {
            let code = try await authorize()
            let session = try await exchange(code: code)
            await tokenStore.set(session.token)
            await store.set(.signedIn(session.account))
            return .success(session.account)
        } catch {
            return .failure(error)
        }
    }

    // MARK: - OAuth (interactive)

    @MainActor
    private func authorize() async throws -> String {
        let authURL = apiBaseURL.appendingPathComponent("auth/google")
        let callback = try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<URL, Error>) in
            let webSession = ASWebAuthenticationSession(url: authURL, callbackURLScheme: callbackScheme) { url, error in
                if let url { continuation.resume(returning: url) }
                else { continuation.resume(throwing: error ?? AuthError.cancelled) }
            }
            #if canImport(UIKit)
            webSession.presentationContextProvider = self
            #endif
            webSession.prefersEphemeralWebBrowserSession = false
            if !webSession.start() { continuation.resume(throwing: AuthError.cancelled) }
        }
        guard let code = Self.extractCode(from: callback) else { throw AuthError.noCode }
        return code
    }

    private func exchange(code: String) async throws -> Session {
        var request = URLRequest(url: apiBaseURL.appendingPathComponent("auth/google/token"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(["code": code])
        let (data, _) = try await session.data(for: request)
        guard let session = Self.decodeSession(from: data) else { throw AuthError.badResponse }
        return session
    }

    // MARK: - Pure, testable steps

    /// Pull the `code` query parameter out of the OAuth callback URL.
    public static func extractCode(from url: URL) -> String? {
        URLComponents(url: url, resolvingAgainstBaseURL: false)?
            .queryItems?.first(where: { $0.name == "code" })?.value
            .flatMap { $0.isEmpty ? nil : $0 }
    }

    /// A decoded sign-in session — account + bearer token.
    public struct Session: Equatable, Sendable {
        public let account: Account
        public let token: String
    }

    /// Decode the API's auth response (`{ "token": "…", "user": { id, email, name } }`).
    public static func decodeSession(from data: Data) -> Session? {
        guard let response = try? JSONDecoder().decode(AuthResponse.self, from: data) else { return nil }
        return Session(
            account: Account(id: response.user.id, email: response.user.email, displayName: response.user.name),
            token: response.token
        )
    }

    /// Convenience for tests that only assert the account mapping.
    public static func decodeAccount(from data: Data) -> Account? {
        decodeSession(from: data)?.account
    }

    private struct AuthResponse: Decodable {
        struct User: Decodable { let id: String; let email: String; let name: String }
        let token: String
        let user: User
    }

    enum AuthError: Error { case cancelled, noCode, badResponse }
}

#if canImport(UIKit)
import UIKit
extension GoogleAuthRepository: ASWebAuthenticationPresentationContextProviding {
    public func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        UIApplication.shared.connectedScenes
            .compactMap { ($0 as? UIWindowScene)?.keyWindow }
            .first ?? ASPresentationAnchor()
    }
}
#endif
