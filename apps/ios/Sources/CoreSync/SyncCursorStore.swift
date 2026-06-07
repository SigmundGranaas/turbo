import Foundation

/// Persists per-entity sync timestamps (`id → updatedAt`) so a record pulled in
/// one session isn't re-pushed in the next. Namespaced per entity type
/// ("markers", "paths", "collections"). Mirrors Android's `SyncCursorStore`.
public protocol SyncCursorStore: Sendable {
    func timestamps(namespace: String) async -> [String: Int64]
    func setTimestamps(_ timestamps: [String: Int64], namespace: String) async
}

public actor InMemoryCursorStore: SyncCursorStore {
    private var store: [String: [String: Int64]] = [:]
    public init() {}
    public func timestamps(namespace: String) -> [String: Int64] { store[namespace] ?? [:] }
    public func setTimestamps(_ timestamps: [String: Int64], namespace: String) { store[namespace] = timestamps }
}

/// `UserDefaults`-backed cursor store (JSON per namespace).
public final class UserDefaultsCursorStore: SyncCursorStore {
    private nonisolated(unsafe) let defaults: UserDefaults
    private let prefix: String

    public init(defaults: UserDefaults = .standard, prefix: String = "turbo.sync.cursor.") {
        self.defaults = defaults
        self.prefix = prefix
    }

    public func timestamps(namespace: String) async -> [String: Int64] {
        guard let data = defaults.data(forKey: prefix + namespace),
              let map = try? JSONDecoder().decode([String: Int64].self, from: data) else { return [:] }
        return map
    }

    public func setTimestamps(_ timestamps: [String: Int64], namespace: String) async {
        guard let data = try? JSONEncoder().encode(timestamps) else { return }
        defaults.set(data, forKey: prefix + namespace)
    }
}
