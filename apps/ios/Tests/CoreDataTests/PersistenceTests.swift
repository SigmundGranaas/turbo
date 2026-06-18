import Testing
import Foundation
import SwiftData
import CoreModel
@testable import CoreData

@Suite("Persistence")
struct PersistenceTests {

    // MARK: UserDefaults settings

    @Test("settings survive a new repository instance (UserDefaults-backed)")
    func settingsPersist() async {
        let defaults = UserDefaults(suiteName: "turbo-test-\(UUID().uuidString)")!
        defer { defaults.removePersistentDomain(forName: defaults.description) }

        let repo = UserDefaultsSettingsRepository(defaults: defaults)
        await repo.update { $0.metricUnits = false; $0.themeMode = .dark }

        let reopened = UserDefaultsSettingsRepository(defaults: defaults)
        let settings = await reopened.current()
        #expect(settings.metricUnits == false)
        #expect(settings.themeMode == .dark)
    }

    // MARK: SwiftData markers

    @Test("markers persist across repository instances on a shared container")
    func markersPersist() async throws {
        let container = try TurboPersistence.inMemoryContainer()
        let repo = SwiftDataMarkerRepository(container: container)
        await repo.upsert(Marker(id: "m1", name: "Summit", kind: .mountain, position: LatLng(lat: 1, lng: 2), notes: "n"))

        let reopened = SwiftDataMarkerRepository(container: container)
        let all = await reopened.current()
        #expect(all.count == 1)
        #expect(all[0].name == "Summit")
        #expect(all[0].kind == .mountain)
        #expect(all[0].notes == "n")
    }

    @Test("upsert replaces by id; delete removes")
    func markersUpsertDelete() async throws {
        let container = try TurboPersistence.inMemoryContainer()
        let repo = SwiftDataMarkerRepository(container: container)
        await repo.upsert(Marker(id: "m1", name: "A", kind: .cabin, position: LatLng(lat: 1, lng: 1)))
        await repo.upsert(Marker(id: "m1", name: "B", kind: .cabin, position: LatLng(lat: 1, lng: 1)))
        var all = await repo.current()
        #expect(all.count == 1)
        #expect(all[0].name == "B")
        await repo.delete(id: "m1")
        all = await repo.current()
        #expect(all.isEmpty)
    }

    @Test("paths + collections persist on a shared container")
    func pathsAndCollectionsPersist() async throws {
        let container = try TurboPersistence.inMemoryContainer()
        let paths = SwiftDataPathRepository(container: container)
        await paths.upsert(SavedPath(id: "p1", name: "Loop", path: GeoPath(points: [LatLng(lat: 1, lng: 2)], source: .recording, elevations: [10]), activityKind: .hiking))
        #expect(await SwiftDataPathRepository(container: container).current().count == 1)

        let collections = SwiftDataCollectionRepository(container: container)
        await collections.upsert(MapCollection(id: "c1", name: "Trips", itemCount: 3))
        #expect(await SwiftDataCollectionRepository(container: container).current().count == 1)
    }

    @Test("a followed track round-trips its planned route and phase splits")
    func followedTrackRoundTrips() async throws {
        let container = try TurboPersistence.inMemoryContainer()
        let paths = SwiftDataPathRepository(container: container)
        let followed = SavedPath(
            id: "p-follow", name: "Cabin (followed)",
            path: GeoPath(points: [LatLng(lat: 1, lng: 2), LatLng(lat: 1.1, lng: 2.1)], source: .recording),
            activityKind: .hiking,
            plannedRoute: [LatLng(lat: 1, lng: 2), LatLng(lat: 1.2, lng: 2.2)],
            phaseSplits: [
                PhaseSplit(index: 0, name: "B", crossedAtEpochMs: 1_700_000_100_000, splitDistanceM: 800, splitSeconds: 600),
                PhaseSplit(index: 1, name: "Cabin", crossedAtEpochMs: 1_700_000_700_000, splitDistanceM: 1200, splitSeconds: 540),
            ]
        )
        await paths.upsert(followed)
        let loaded = try #require(await SwiftDataPathRepository(container: container).current().first { $0.id == "p-follow" })
        #expect(loaded.plannedRoute?.count == 2)
        #expect(loaded.plannedRoute?.last?.lng == 2.2)
        #expect(loaded.phaseSplits.count == 2)
        #expect(loaded.phaseSplits[1].name == "Cabin")
        #expect(loaded.phaseSplits[1].splitSeconds == 540)
    }

    @Test("a plain recording has no planned route and empty splits")
    func plainRecordingHasNoFollowMetadata() async throws {
        let container = try TurboPersistence.inMemoryContainer()
        let paths = SwiftDataPathRepository(container: container)
        await paths.upsert(SavedPath(id: "p-rec", name: "Walk", path: GeoPath(points: [LatLng(lat: 1, lng: 2)], source: .recording)))
        let loaded = try #require(await SwiftDataPathRepository(container: container).current().first { $0.id == "p-rec" })
        #expect(loaded.plannedRoute == nil)
        #expect(loaded.phaseSplits.isEmpty)
    }
}
