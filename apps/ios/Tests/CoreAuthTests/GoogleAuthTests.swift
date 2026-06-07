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

    @Test("decodes the API auth response into an Account")
    func decodeAccount() throws {
        let json = #"{ "token": "t", "user": { "id": "u1", "email": "a@b.no", "name": "Ada Berg" } }"#.data(using: .utf8)!
        let account = GoogleAuthRepository.decodeAccount(from: json)
        #expect(account?.id == "u1")
        #expect(account?.email == "a@b.no")
        #expect(account?.displayName == "Ada Berg")
    }

    @Test("returns nil for a malformed response")
    func decodeFailure() {
        #expect(GoogleAuthRepository.decodeAccount(from: Data("nope".utf8)) == nil)
    }

    @Test("in-memory auth signs in and out")
    func inMemoryFlow() async {
        let auth = InMemoryAuthRepository()
        #expect(await auth.current() == .signedOut)
        _ = await auth.signIn()
        #expect(await auth.current().account != nil)
        await auth.signOut()
        #expect(await auth.current() == .signedOut)
    }
}
