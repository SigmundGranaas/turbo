import Foundation
import Testing
import CoreModel

/// Loads the shared cross-platform tracking fixtures from `fixtures/tracking/` at
/// the repo root (the same files the Android tests load). Host-only — these are
/// pure-logic tests run via `swift test`, which can read the repo on disk.
enum TrackingFixtures {

    struct ProgressFixture: Decodable {
        let name: String
        let params: Params
        let route: [[Double]]
        let fixes: [[Double]]
        let expect: [Expect]

        struct Params: Decodable {
            let windowBackM: Double
            let windowAheadM: Double
            let offRouteM: Double
            let arriveEndM: Double
        }
        struct Expect: Decodable {
            let fraction: Double
            let arrived: Bool
            let offRoute: Bool
        }

        var routePoints: [LatLng] { route.map { LatLng(lat: $0[0], lng: $0[1]) } }
        var fixPoints: [LatLng] { fixes.map { LatLng(lat: $0[0], lng: $0[1]) } }
    }

    static func progress(_ name: String) -> ProgressFixture {
        decode("progress/\(name).json")
    }

    private static func decode<T: Decodable>(_ relativePath: String) -> T {
        let url = root().appendingPathComponent(relativePath)
        guard let data = try? Data(contentsOf: url) else {
            fatalError("fixture not readable: \(url.path)")
        }
        // swiftlint:disable:next force_try
        return try! JSONDecoder().decode(T.self, from: data)
    }

    /// Walk up from this source file until `fixtures/tracking` is found.
    private static func root() -> URL {
        var dir = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        while dir.path != "/" {
            let candidate = dir.appendingPathComponent("fixtures/tracking")
            if FileManager.default.fileExists(atPath: candidate.path) { return candidate }
            dir = dir.deletingLastPathComponent()
        }
        fatalError("fixtures/tracking not found above \(#filePath)")
    }
}

@Suite("Tracking fixtures load")
struct TrackingFixtureLoadingTests {
    @Test("progress fixtures parse with matching expect counts")
    func loadProgress() {
        let straight = TrackingFixtures.progress("straight")
        #expect(straight.route.count == 2)
        #expect(straight.fixes.count == straight.expect.count)
        #expect(straight.params.windowAheadM == 400)

        let oab = TrackingFixtures.progress("out-and-back")
        #expect(oab.route.count == 3)
        #expect(oab.fixes.count == 9)
        #expect(oab.expect.last?.arrived == true)
    }
}
