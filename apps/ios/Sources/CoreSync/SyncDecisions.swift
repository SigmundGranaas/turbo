import Foundation

/// The outcome of reconciling local and remote record sets: what to apply to the
/// local store, and what to push to the server.
public struct Reconciliation<Payload: Codable & Sendable & Equatable>: Sendable, Equatable {
    public let applyLocally: [SyncRecord<Payload>]
    public let pushRemote: [SyncRecord<Payload>]
}

/// Pure last-write-wins reconciliation — the heart of the sync engine, kept free
/// of I/O so it's exhaustively unit-testable. Mirrors `core.sync.SyncDecisions`.
public enum SyncDecisions {

    /// Compare `local` and `remote` records (keyed by id). For each id the newer
    /// `updatedAt` wins; ties are no-ops. Records the loser side must adopt land in
    /// the corresponding bucket (`applyLocally` for remote-wins, `pushRemote` for
    /// local-wins / local-only).
    public static func reconcile<P>(
        local: [SyncRecord<P>],
        remote: [SyncRecord<P>]
    ) -> Reconciliation<P> {
        let localByID = Dictionary(local.map { ($0.id, $0) }, uniquingKeysWith: { a, b in a.updatedAt >= b.updatedAt ? a : b })
        let remoteByID = Dictionary(remote.map { ($0.id, $0) }, uniquingKeysWith: { a, b in a.updatedAt >= b.updatedAt ? a : b })

        var applyLocally: [SyncRecord<P>] = []
        var pushRemote: [SyncRecord<P>] = []

        for id in Set(localByID.keys).union(remoteByID.keys) {
            switch (localByID[id], remoteByID[id]) {
            case let (.some(l), .none):
                pushRemote.append(l)
            case let (.none, .some(r)):
                applyLocally.append(r)
            case let (.some(l), .some(r)):
                if r.updatedAt > l.updatedAt { applyLocally.append(r) }
                else if l.updatedAt > r.updatedAt { pushRemote.append(l) }
            case (.none, .none):
                break
            }
        }
        return Reconciliation(
            applyLocally: applyLocally.sorted { $0.id < $1.id },
            pushRemote: pushRemote.sorted { $0.id < $1.id }
        )
    }
}
