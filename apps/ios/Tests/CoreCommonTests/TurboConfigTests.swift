import Testing
import Foundation
@testable import CoreCommon

@Suite("TurboConfig")
struct TurboConfigTests {

    @Test("a valid base URL means online")
    func online() {
        let config = TurboConfig.from(["TurboAPIBaseURL": "https://kart.sandring.no/api"])
        #expect(config.isOnline)
        #expect(config.apiBaseURL?.absoluteString == "https://kart.sandring.no/api")
    }

    @Test("missing or blank URL means offline (safe default)")
    func offline() {
        #expect(TurboConfig.from(nil).isOnline == false)
        #expect(TurboConfig.from([:]).isOnline == false)
        #expect(TurboConfig.from(["TurboAPIBaseURL": ""]).isOnline == false)
        #expect(TurboConfig.from(["TurboAPIBaseURL": "   "]).isOnline == false)
    }
}
