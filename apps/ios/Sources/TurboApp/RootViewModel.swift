import Foundation
import Observation
import SwiftUI
import CoreModel
import CoreData
import CoreAuth

/// App-level state that outlives any one screen: the persisted theme mode (so the
/// whole app re-themes from Settings) and the current account (for the avatar /
/// account menu). Mirrors the bits `MainActivity` reads on Android.
@MainActor
@Observable
public final class RootViewModel {
    public private(set) var themeMode: ThemeMode = .system
    public private(set) var account: Account?

    private let settingsRepository: SettingsRepository
    private let authRepository: AuthRepository
    private var observations: [Task<Void, Never>] = []

    public init(settingsRepository: SettingsRepository, authRepository: AuthRepository) {
        self.settingsRepository = settingsRepository
        self.authRepository = authRepository
    }

    public func start() {
        guard observations.isEmpty else { return }
        observations.append(Task { [weak self, settingsRepository] in
            for await settings in await settingsRepository.stream() {
                self?.themeMode = settings.themeMode
            }
        })
        observations.append(Task { [weak self, authRepository] in
            for await state in await authRepository.state() {
                self?.account = state.account
            }
        })
    }

    /// The SwiftUI scheme to force, or `nil` to follow the system.
    public var colorScheme: ColorScheme? {
        switch themeMode {
        case .system: nil
        case .light: .light
        case .dark: .dark
        }
    }
}
