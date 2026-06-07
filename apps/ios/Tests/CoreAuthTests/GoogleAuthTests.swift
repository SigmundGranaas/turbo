import Testing
import Foundation
@testable import CoreAuth

@Suite("Google auth parsing")
struct GoogleAuthTests {

    @Test("extracts the code from the OAuth callback URL")
    func extractCode() {
        let url = URL(string: "turbo://oauth?code=abc123&state=xyz")!
        #expect(GoogleAuthRepository.extractCode(from: url) == "abc123")
    }

    @Test("returns nil when there's no code")
    func noCode() {
        #expect(GoogleAuthRepository.extractCode(from: URL(string: "turbo://oauth?error=denied")!) == nil)
        #expect(GoogleAuthRepository.extractCode(from: URL(string: "turbo://oauth?code=")!) == nil)
    }

    @Test("decodes the API auth response into an account + token")
    func decodeSession() throws {
        let json = #"{ "token": "jwt-123", "user": { "id": "u1", "email": "a@b.no", "name": "Ada Berg" } }"#.data(using: .utf8)!
        let session = GoogleAuthRepository.decodeSession(from: json)
        #expect(session?.account.id == "u1")
        #expect(session?.account.displayName == "Ada Berg")
        #expect(session?.token == "jwt-123")
    }

    @Test("returns nil for a malformed response")
    func decodeFailure() {
        #expect(GoogleAuthRepository.decodeAccount(from: Data("nope".utf8)) == nil)
    }

    @Test("in-memory auth signs in and out, and vends a token only when signed in")
    func inMemoryFlow() async {
        let auth = InMemoryAuthRepository()
        #expect(await auth.current() == .signedOut)
        #expect(await auth.token() == nil)
        _ = await auth.signIn()
        #expect(await auth.current().account != nil)
        #expect(await auth.token() != nil)
        await auth.signOut()
        #expect(await auth.current() == .signedOut)
        #expect(await auth.token() == nil)
    }
}
