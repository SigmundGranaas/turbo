import Foundation
import CoreModel
import CoreCommon

/// `UserDefaults`-backed ``SettingsRepository`` — the iOS analogue of Android's
/// DataStore. `UserSettings` is `Codable`, persisted as JSON under one key; an
/// in-memory ``ReactiveStore`` fronts it so reads/streams stay synchronous-fast.
public final class UserDefaultsSettingsRepository: SettingsRepository {
    private let store: ReactiveStore<UserSettings>
    /// `UserDefaults` is documented thread-safe but not `Sendable`.
    private nonisolated(unsafe) let defaults: UserDefaults
    private let key: String

    public init(defaults: UserDefaults = .standard, key: String = "turbo.settings") {
        self.defaults = defaults
        self.key = key
        let initial = Self.load(from: defaults, key: key) ?? UserSettings()
        self.store = ReactiveStore(initial)
    }

    public func current() async -> UserSettings { await store.current() }
    public func stream() async -> AsyncStream<UserSettings> { await store.stream() }

    public func update(_ transform: @Sendable (inout UserSettings) -> Void) async {
        await store.update { settings in
            var next = settings
            transform(&next)
            return next
        }
        persist(await store.current())
    }

    private func persist(_ settings: UserSettings) {
        guard let data = try? JSONEncoder().encode(settings) else { return }
        defaults.set(data, forKey: key)
    }

    private static func load(from defaults: UserDefaults, key: String) -> UserSettings? {
        guard let data = defaults.data(forKey: key) else { return nil }
        return try? JSONDecoder().decode(UserSettings.self, from: data)
    }
}
