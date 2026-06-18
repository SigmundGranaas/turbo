import Foundation
import Testing
import CoreModel
@testable import CoreData

/// Loads the shared `fixtures/tracking/filter/*` and runs them through
/// `LocationFilter`, asserting the same accepted indices the Android test does.
@Suite("LocationFilter")
struct LocationFilterTests {

    private struct FilterFixture: Decodable {
        let params: Params
        let fixes: [Fix]
        let acceptedIndices: [Int]
        struct Params: Decodable { let accuracyMaxM, stalenessMaxMs, jumpMaxM: Double }
        struct Fix: Decodable { let lat, lng, accuracyM, ageMs: Double }
    }

    private func fixture(_ name: String) -> FilterFixture {
        var dir = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        while dir.path != "/" {
            let candidate = dir.appendingPathComponent("fixtures/tracking/filter/\(name).json")
            if FileManager.default.fileExists(atPath: candidate.path) {
                // swiftlint:disable:next force_try
                return try! JSONDecoder().decode(FilterFixture.self, from: try! Data(contentsOf: candidate))
            }
            dir = dir.deletingLastPathComponent()
        }
        fatalError("filter fixture \(name) not found")
    }

    private func run(_ name: String) {
        let fx = fixture(name)
        let filter = LocationFilter(accuracyMaxM: fx.params.accuracyMaxM,
                                    stalenessMaxMs: fx.params.stalenessMaxMs,
                                    jumpMaxM: fx.params.jumpMaxM)
        var accepted: [Int] = []
        for (i, f) in fx.fixes.enumerated() {
            if filter.accept(position: LatLng(lat: f.lat, lng: f.lng), accuracyM: f.accuracyM, ageMs: f.ageMs) {
                accepted.append(i)
            }
        }
        #expect(accepted == fx.acceptedIndices, "\(name): accepted \(accepted) != \(fx.acceptedIndices)")
    }

    @Test("valid walk — all accepted") func validWalk() { run("valid-walk") }
    @Test("stale resume fix dropped") func resumeStale() { run("resume-stale") }
    @Test("low-accuracy fix dropped") func lowAccuracy() { run("low-accuracy") }
    @Test("isolated teleport rejected; confirmed jump accepted") func teleport() { run("teleport-jump") }
}
