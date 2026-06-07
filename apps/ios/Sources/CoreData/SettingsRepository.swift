import Foundation
import CoreModel
import CoreCommon

/// Persisted user preferences. Mirrors `core.data.SettingsRepository`
/// (DataStore-backed on Android; in-memory here, UserDefaults/SwiftData later).
public protocol SettingsRepository: Sendable {
    func current() async -> UserSettings
    func stream() async -> AsyncStream<UserSettings>
    func update(_ transform: @Sendable (inout UserSettings) -> Void) async
}

public final class InMemorySettingsRepository: SettingsRepository {
    private let store: ReactiveStore<UserSettings>

    public init(initial: UserSettings = UserSettings()) {
        store = ReactiveStore(initial)
    }

    public func current() async -> UserSettings { await store.current() }
    public func stream() async -> AsyncStream<UserSettings> { await store.stream() }

    public func update(_ transform: @Sendable (inout UserSettings) -> Void) async {
        await store.update { settings in
            var next = settings
            transform(&next)
            return next
        }
    }
}
