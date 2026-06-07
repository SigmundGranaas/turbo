import Testing
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

    @Test("a local-only record is pushed to remote")
    func localOnly() {
        let r = SyncDecisions.reconcile(local: [record("a", 1)], remote: [])
        #expect(r.pushRemote.map(\.id) == ["a"])
        #expect(r.applyLocally.isEmpty)
    }

    @Test("a remote-only record is applied locally")
    func remoteOnly() {
        let r = SyncDecisions.reconcile(local: [], remote: [record("b", 1)])
        #expect(r.applyLocally.map(\.id) == ["b"])
        #expect(r.pushRemote.isEmpty)
    }

    @Test("the newer side wins a conflict")
    func conflict() {
        let newerRemote = SyncDecisions.reconcile(local: [record("x", 1)], remote: [record("x", 2)])
        #expect(newerRemote.applyLocally.map(\.id) == ["x"])
        #expect(newerRemote.pushRemote.isEmpty)

        let newerLocal = SyncDecisions.reconcile(local: [record("x", 5)], remote: [record("x", 2)])
        #expect(newerLocal.pushRemote.map(\.id) == ["x"])
        #expect(newerLocal.applyLocally.isEmpty)
    }

    @Test("a newer delete tombstone wins and is applied locally")
    func tombstone() {
        let r = SyncDecisions.reconcile(local: [record("x", 1)], remote: [record("x", 9, deleted: true)])
        #expect(r.applyLocally.count == 1)
        #expect(r.applyLocally[0].deleted)
    }

    @Test("equal timestamps are a no-op")
    func tie() {
        let r = SyncDecisions.reconcile(local: [record("x", 3)], remote: [record("x", 3)])
        #expect(r.applyLocally.isEmpty)
        #expect(r.pushRemote.isEmpty)
    }
}

@Suite("MarkerSyncEngine")
struct MarkerSyncEngineTests {

    private func marker(_ id: String) -> Marker {
        Marker(id: id, name: id, kind: .mountain, position: LatLng(lat: 1, lng: 2))
    }

    @Test("local-only markers are pushed to the server")
    func pushesLocal() async throws {
        let repo = InMemoryMarkerRepository(seed: [marker("local1")])
        let transport = InMemoryMarkerSyncTransport()
        let engine = MarkerSyncEngine(repository: repo, transport: transport, now: { 100 })
        try await engine.sync()
        let remote = await transport.pull()
        #expect(remote.map(\.id) == ["local1"])
    }

    @Test("remote-only markers are pulled into the local repository")
    func pullsRemote() async throws {
        let repo = InMemoryMarkerRepository(seed: [])
        let remoteRecord = SyncRecord(id: "remote1", updatedAt: 50, deleted: false, payload: MarkerPayload(marker("remote1")))
        let transport = InMemoryMarkerSyncTransport(seed: [remoteRecord])
        let engine = MarkerSyncEngine(repository: repo, transport: transport, now: { 100 })
        try await engine.sync()
        #expect(await repo.current().contains { $0.id == "remote1" })
    }

    @Test("disjoint local + remote converge on both sides")
    func converges() async throws {
        let repo = InMemoryMarkerRepository(seed: [marker("L")])
        let transport = InMemoryMarkerSyncTransport(seed: [
            SyncRecord(id: "R", updatedAt: 10, deleted: false, payload: MarkerPayload(marker("R")))
        ])
        let engine = MarkerSyncEngine(repository: repo, transport: transport, now: { 100 })
        try await engine.sync()
        let localIDs = Set(await repo.current().map(\.id))
        let remoteIDs = Set(await transport.pull().map(\.id))
        #expect(localIDs == ["L", "R"])
        #expect(remoteIDs == ["L", "R"])
    }
}

@Suite("SyncController gating")
struct SyncControllerTests {

    @Test("does nothing when signed out")
    func signedOut() async {
        let repo = InMemoryMarkerRepository(seed: [Marker(id: "a", name: "a", kind: .mountain, position: LatLng(lat: 0, lng: 0))])
        let transport = InMemoryMarkerSyncTransport()
        let controller = SyncController(
            engine: MarkerSyncEngine(repository: repo, transport: transport, now: { 1 }),
            auth: InMemoryAuthRepository(initial: .signedOut),
            settings: InMemorySettingsRepository()
        )
        await controller.syncNow()
        #expect(await transport.pull().isEmpty)   // nothing pushed
    }

    @Test("syncs when signed in and enabled")
    func signedIn() async {
        let repo = InMemoryMarkerRepository(seed: [Marker(id: "a", name: "a", kind: .mountain, position: LatLng(lat: 0, lng: 0))])
        let transport = InMemoryMarkerSyncTransport()
        let auth = InMemoryAuthRepository()
        _ = await auth.signIn()
        let controller = SyncController(
            engine: MarkerSyncEngine(repository: repo, transport: transport, now: { 1 }),
            auth: auth,
            settings: InMemorySettingsRepository()
        )
        await controller.syncNow()
        #expect(await transport.pull().map(\.id) == ["a"])
    }
}
