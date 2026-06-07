import Foundation

/// A non-generic handle so ``SyncController`` can run heterogeneous engines.
public protocol SyncUnit: Sendable {
    func sync() async throws
}

/// Bidirectional last-write-wins sync for one entity type. Maps the local store
/// to records (stamping first-seen ids via the persisted ``SyncCursorStore`),
/// reconciles against the transport, applies remote wins, and pushes local wins.
/// Generic over the wire ``Payload`` — markers, paths and collections all reuse it.
public actor EntitySyncEngine<Payload: Codable & Sendable & Equatable>: SyncUnit {
    private let namespace: String
    private let transport: any SyncTransport<Payload>
    private let cursor: SyncCursorStore
    private let loadLocal: @Sendable () async -> [(id: String, payload: Payload)]
    private let applyUpsert: @Sendable (String, Payload) async -> Void
    private let applyDelete: @Sendable (String) async -> Void
    private let now: @Sendable () -> Int64

    public init(
        namespace: String,
        transport: any SyncTransport<Payload>,
        cursor: SyncCursorStore,
        loadLocal: @escaping @Sendable () async -> [(id: String, payload: Payload)],
        applyUpsert: @escaping @Sendable (String, Payload) async -> Void,
        applyDelete: @escaping @Sendable (String) async -> Void,
        now: @escaping @Sendable () -> Int64 = { Int64(Date().timeIntervalSince1970 * 1000) }
    ) {
        self.namespace = namespace
        self.transport = transport
        self.cursor = cursor
        self.loadLocal = loadLocal
        self.applyUpsert = applyUpsert
        self.applyDelete = applyDelete
        self.now = now
    }

    public func sync() async throws {
        var timestamps = await cursor.timestamps(namespace: namespace)

        let local = await loadLocal().map { pair -> SyncRecord<Payload> in
            let stamp = timestamps[pair.id] ?? now()
            timestamps[pair.id] = stamp
            return SyncRecord(id: pair.id, updatedAt: stamp, deleted: false, payload: pair.payload)
        }

        let remote = try await transport.pull()
        let result = SyncDecisions.reconcile(local: local, remote: remote)

        for record in result.applyLocally {
            timestamps[record.id] = record.updatedAt
            if record.deleted {
                await applyDelete(record.id)
            } else if let payload = record.payload {
                await applyUpsert(record.id, payload)
            }
        }

        try await transport.push(result.pushRemote)
        await cursor.setTimestamps(timestamps, namespace: namespace)
    }
}
