import Foundation
import Observation
import CoreModel
import CoreData

/// Settings UI state is the persisted ``UserSettings``. Mirrors
/// `feature.settings.SettingsViewModel` (Android).
@MainActor
@Observable
public final class SettingsViewModel {
    public private(set) var settings = UserSettings()

    private let repository: SettingsRepository
    private var observation: Task<Void, Never>?

    public init(repository: SettingsRepository) {
        self.repository = repository
    }

    public func start() {
        guard observation == nil else { return }
        observation = Task { [weak self, repository] in
            for await value in await repository.stream() {
                self?.settings = value
            }
        }
    }

    public func stop() { observation?.cancel(); observation = nil }

    public func setMetricUnits(_ value: Bool) { mutate { $0.metricUnits = value } }
    public func setCompassOrientation(_ value: Bool) { mutate { $0.compassOrientation = value } }
    public func setFollowLocation(_ value: Bool) { mutate { $0.followLocation = value } }
    public func setThemeMode(_ value: ThemeMode) { mutate { $0.themeMode = value } }
    public func setShareLocation(_ value: Bool) { mutate { $0.shareLocation = value } }
    public func setAvalancheAlerts(_ value: Bool) { mutate { $0.avalancheAlerts = value } }

    private func mutate(_ transform: @escaping @Sendable (inout UserSettings) -> Void) {
        Task { [repository] in await repository.update(transform) }
    }
}
