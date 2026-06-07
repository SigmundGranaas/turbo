import Foundation
import CoreModel
import CoreData

/// Builds the per-entity ``EntitySyncEngine``s from the repositories — the wiring
/// that maps each repo to the shared sync machinery.
public enum Syncers {

    public static func marker(
        repository: MarkerRepository,
        transport: any SyncTransport<MarkerPayload>,
        cursor: SyncCursorStore,
        now: @escaping @Sendable () -> Int64 = { Int64(Date().timeIntervalSince1970 * 1000) }
    ) -> EntitySyncEngine<MarkerPayload> {
        EntitySyncEngine(
            namespace: "markers", transport: transport, cursor: cursor,
            loadLocal: { await repository.current().map { ($0.id, MarkerPayload($0)) } },
            applyUpsert: { id, payload in await repository.upsert(payload.marker(id: id)) },
            applyDelete: { id in await repository.delete(id: id) },
            now: now
        )
    }

    public static func path(
        repository: PathRepository,
        transport: any SyncTransport<PathPayload>,
        cursor: SyncCursorStore,
        now: @escaping @Sendable () -> Int64 = { Int64(Date().timeIntervalSince1970 * 1000) }
    ) -> EntitySyncEngine<PathPayload> {
        EntitySyncEngine(
            namespace: "paths", transport: transport, cursor: cursor,
            loadLocal: { await repository.current().map { ($0.id, PathPayload($0)) } },
            applyUpsert: { id, payload in await repository.upsert(payload.savedPath(id: id)) },
            applyDelete: { id in await repository.delete(id: id) },
            now: now
        )
    }

    public static func collection(
        repository: CollectionRepository,
        transport: any SyncTransport<CollectionPayload>,
        cursor: SyncCursorStore,
        now: @escaping @Sendable () -> Int64 = { Int64(Date().timeIntervalSince1970 * 1000) }
    ) -> EntitySyncEngine<CollectionPayload> {
        EntitySyncEngine(
            namespace: "collections", transport: transport, cursor: cursor,
            loadLocal: { await repository.current().map { ($0.id, CollectionPayload($0)) } },
            applyUpsert: { id, payload in await repository.upsert(payload.collection(id: id)) },
            applyDelete: { id in await repository.delete(id: id) },
            now: now
        )
    }
}
