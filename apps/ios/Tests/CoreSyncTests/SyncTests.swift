import Testing
import Foundation
import CoreModel
import CoreData
import CoreAuth
@testable import CoreSync

@Suite("SyncDecisions (last-write-wins)")
struct SyncDecisionsTests {

    private func record(_ id: String, _ updatedAt: Int64, deleted: Bool = false) -> SyncRecord<MarkerPayload> {
        SyncRecord(id: id, updatedAt: updatedAt, deleted: deleted,
                   payload: deleted ? nil : MarkerPayload(Marker(id: id, name: id, kind: .mountain, position: LatLng(lat: 0, lng: 0))))
    }

    @Test("local-only is pushed; remote-only is applied")
    func disjoint() {
        let a = SyncDecisions.reconcile(local: [record("a", 1)], remote: [])
        #expect(a.pushRemote.map(\.id) == ["a"])
        let b = SyncDecisions.reconcile(local: [], remote: [record("b", 1)])
        #expect(b.applyLocally.map(\.id) == ["b"])
    }

    @Test("the newer side wins a conflict")
    func conflict() {
        #expect(SyncDecisions.reconcile(local: [record("x", 1)], remote: [record("x", 2)]).applyLocally.map(\.id) == ["x"])
        #expect(SyncDecisions.reconcile(local: [record("x", 5)], remote: [record("x", 2)]).pushRemote.map(\.id) == ["x"])
    }

    @Test("a newer delete tombstone wins")
    func tombstone() {
        let r = SyncDecisions.reconcile(local: [record("x", 1)], remote: [record("x", 9, deleted: true)])
        #expect(r.applyLocally.first?.deleted == true)
    }
}

@Suite("Sync cursor store")
struct SyncCursorStoreTests {
    @Test("UserDefaults cursor round-trips per namespace")
    func roundTrip() async {
        let defaults = UserDefaults(suiteName: "sync-test-\(UUID().uuidString)")!
        let store = UserDefaultsCursorStore(defaults: defaults)
        await store.setTimestamps(["a": 10, "b": 20], namespace: "markers")
        #expect(await store.timestamps(namespace: "markers") == ["a": 10, "b": 20])
        #expect(await store.timestamps(namespace: "paths").isEmpty)
        // survives a new instance
        let reopened = UserDefaultsCursorStore(defaults: defaults)
        #expect(await reopened.timestamps(namespace: "markers")["a"] == 10)
    }
}

@Suite("EntitySyncEngine")
struct EntitySyncEngineTests {

    private func marker(_ id: String) -> Marker {
        Marker(id: id, name: id, kind: .mountain, position: LatLng(lat: 1, lng: 2))
    }

    @Test("markers: local pushes, remote pulls, disjoint sets converge")
    func markersConverge() async throws {
        let repo = InMemoryMarkerRepository(seed: [marker("L")])
        let transport = InMemorySyncTransport<MarkerPayload>(seed: [
            SyncRecord(id: "R", updatedAt: 10, deleted: false, payload: MarkerPayload(marker("R")))
        ])
        let engine = Syncers.marker(repository: repo, transport: transport, cursor: InMemoryCursorStore(), now: { 100 })
        try await engine.sync()
        #expect(Set(await repo.current().map(\.id)) == ["L", "R"])
        #expect(Set(await transport.pull().map(\.id)) == ["L", "R"])
    }

    @Test("paths sync through the same engine")
    func pathsSync() async throws {
        let repo = InMemoryPathRepository(seed: [])
        let payload = PathPayload(SavedPath(id: "p1", name: "Loop",
            path: GeoPath(points: [LatLng(lat: 1, lng: 2)], source: .recording, elevations: [10])))
        let transport = InMemorySyncTransport<PathPayload>(seed: [
            SyncRecord(id: "p1", updatedAt: 5, deleted: false, payload: payload)
        ])
        let engine = Syncers.path(repository: repo, transport: transport, cursor: InMemoryCursorStore(), now: { 100 })
        try await engine.sync()
        #expect(await repo.current().contains { $0.id == "p1" && $0.name == "Loop" })
    }

    @Test("a persisted cursor stops a pulled record being re-pushed next run")
    func cursorPreventsRePush() async throws {
        let repo = InMemoryMarkerRepository(seed: [])
        let transport = InMemorySyncTransport<MarkerPayload>(seed: [
            SyncRecord(id: "R", updatedAt: 10, deleted: false, payload: MarkerPayload(marker("R")))
        ])
        let cursor = InMemoryCursorStore()
        // First sync pulls R (updatedAt 10) and records its timestamp.
        try await Syncers.marker(repository: repo, transport: transport, cursor: cursor, now: { 100 }).sync()
        #expect(await cursor.timestamps(namespace: "markers")["R"] == 10)
        // A fresh engine sharing the cursor must NOT bump R to "now" and re-push it.
        try await Syncers.marker(repository: repo, transport: transport, cursor: cursor, now: { 999 }).sync()
        let remote = await transport.pull().first { $0.id == "R" }
        #expect(remote?.updatedAt == 10)
    }
}

@Suite("SyncController gating")
struct SyncControllerTests {

    private func controller(auth: AuthRepository, transport: InMemorySyncTransport<MarkerPayload>) -> SyncController {
        let repo = InMemoryMarkerRepository(seed: [Marker(id: "a", name: "a", kind: .mountain, position: LatLng(lat: 0, lng: 0))])
        return SyncController(
            units: [Syncers.marker(repository: repo, transport: transport, cursor: InMemoryCursorStore(), now: { 1 })],
            auth: auth, settings: InMemorySettingsRepository()
        )
    }

    @Test("does nothing when signed out")
    func signedOut() async {
        let transport = InMemorySyncTransport<MarkerPayload>()
        await controller(auth: InMemoryAuthRepository(initial: .signedOut), transport: transport).syncNow()
        #expect(await transport.pull().isEmpty)
    }

    @Test("syncs when signed in")
    func signedIn() async {
        let transport = InMemorySyncTransport<MarkerPayload>()
        let auth = InMemoryAuthRepository()
        _ = await auth.signIn()
        await controller(auth: auth, transport: transport).syncNow()
        #expect(await transport.pull().map(\.id) == ["a"])
    }

    @Test("a clean pass reports synced; a failing unit reports failed")
    @MainActor
    func statusReporting() async {
        let auth = InMemoryAuthRepository()
        _ = await auth.signIn()

        let okStatus = SyncStatus()
        let ok = SyncController(units: [StubUnit(fails: false)], auth: auth,
                                settings: InMemorySettingsRepository(), status: okStatus, now: { Date(timeIntervalSince1970: 100) })
        await ok.syncNow()
        #expect(okStatus.phase == .synced)
        #expect(okStatus.lastSyncedAt == Date(timeIntervalSince1970: 100))

        let badStatus = SyncStatus()
        let bad = SyncController(units: [StubUnit(fails: true)], auth: auth,
                                 settings: InMemorySettingsRepository(), status: badStatus)
        await bad.syncNow()
        #expect(badStatus.isFailed)
    }
}

private struct StubUnit: SyncUnit {
    let fails: Bool
    func sync() async throws {
        if fails { throw NSError(domain: "test", code: 1) }
    }
}
