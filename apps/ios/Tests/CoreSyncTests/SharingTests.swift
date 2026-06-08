import Testing
import Foundation
import CoreCommon
@testable import CoreSync

@Suite("Sharing")
struct SharingTests {

    @Test("friend code is prefixed; link URL is web-base/link/token")
    func formatting() {
        #expect(HttpSharingRepository.formatFriendCode("AB12CD") == "turbo-AB12CD")
        let url = HttpSharingRepository.linkURL(webBase: URL(string: "https://kart.sandring.no")!, token: "xyz")
        #expect(url.absoluteString == "https://kart.sandring.no/link/xyz")
    }

    @Test("redemption decodes resource id / type / role")
    func redemption() {
        let json = #"{"resourceId":"m1","resourceType":"marker","role":"viewer"}"#.data(using: .utf8)!
        let r = HttpSharingRepository.decodeRedemption(json)
        #expect(r?.resourceId == "m1")
        #expect(r?.resourceType == "marker")
        #expect(r?.role == "viewer")
    }

    @Test("in-memory double round-trips a created link")
    func roundTrip() async {
        let repo = InMemorySharingRepository()
        #expect(await repo.friendCode().getOrNil() == "turbo-TEST01")
        let url = await repo.createLink(resourceId: "m1").getOrNil()
        #expect(url != nil)
        let token = url!.lastPathComponent
        let redemption = await repo.redeemLink(token: token).getOrNil()
        #expect(redemption?.resourceId == "m1")
    }

    @Test("redeeming an unknown token fails")
    func unknownToken() async {
        let repo = InMemorySharingRepository()
        if case .success = await repo.redeemLink(token: "nope") { Issue.record("should fail") }
    }
}
