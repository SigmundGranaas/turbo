import Foundation
import CoreCommon

/// The signed-in account. Mirrors `core.auth.Account`.
public struct Account: Equatable, Sendable {
    public let id: String
    public let email: String
    public let displayName: String
    public init(id: String, email: String, displayName: String) {
        self.id = id
        self.email = email
        self.displayName = displayName
    }
}

/// Authentication state. Mirrors `core.auth.AuthState`.
public enum AuthState: Equatable, Sendable {
    case signedOut
    case signedIn(Account)

    public var account: Account? {
        if case let .signedIn(account) = self { account } else { nil }
    }
}

/// Owns sign-in/out and the observable ``AuthState``. Mirrors `core.auth.AuthRepository`.
/// The in-memory implementation fakes a successful Sign in with Apple; the real
/// one (ASAuthorization + token store) swaps in behind this protocol.
public protocol AuthRepository: Sendable {
    func state() async -> AsyncStream<AuthState>
    func current() async -> AuthState
    /// Begin the provider's sign-in flow (Google). Returns the signed-in account.
    func signIn() async -> Outcome<Account>
    func signOut() async
}

public final class InMemoryAuthRepository: AuthRepository {
    private let store: ReactiveStore<AuthState>

    public init(initial: AuthState = .signedOut) {
        store = ReactiveStore(initial)
    }

    public func state() async -> AsyncStream<AuthState> { await store.stream() }
    public func current() async -> AuthState { await store.current() }

    public func signIn() async -> Outcome<Account> {
        let account = Account(id: "local", email: "sigmund@granaas.no", displayName: "Sigmund Granaas")
        await store.set(.signedIn(account))
        return .success(account)
    }

    public func signOut() async {
        await store.set(.signedOut)
    }
}
