import Foundation
import CoreModel
import CoreData

/// Bidirectional marker sync: maps the local ``MarkerRepository`` to records,
/// reconciles against the transport via ``SyncDecisions``, applies remote wins
/// locally and pushes local wins. Mirrors `core.sync.MarkerSync` (Android).
///
/// Per-id `updatedAt` is tracked in-memory (persist via a cursor store later);
/// a markers's first sighting is stamped `now()` so local-only markers push.
public actor MarkerSyncEngine {
    private let repository: MarkerRepository
    private let transport: MarkerSyncTransport
    private let now: @Sendable () -> Int64
    private var timestamps: [String: Int64] = [:]

    public init(
        repository: MarkerRepository,
        transport: MarkerSyncTransport,
        now: @escaping @Sendable () -> Int64 = { Int64(Date().timeIntervalSince1970 * 1000) }
    ) {
        self.repository = repository
        self.transport = transport
        self.now = now
    }

    public func sync() async throws {
        let markers = await repository.current()
        let local = markers.map { marker -> SyncRecord<MarkerPayload> in
            let stamp = timestamps[marker.id] ?? now()
            timestamps[marker.id] = stamp
            return SyncRecord(id: marker.id, updatedAt: stamp, deleted: false, payload: MarkerPayload(marker))
        }

        let remote = try await transport.pull()
        let result = SyncDecisions.reconcile(local: local, remote: remote)

        for record in result.applyLocally {
            timestamps[record.id] = record.updatedAt
            if record.deleted {
                await repository.delete(id: record.id)
            } else if let payload = record.payload {
                await repository.upsert(payload.marker(id: record.id))
            }
        }

        try await transport.push(result.pushRemote)
    }
}
