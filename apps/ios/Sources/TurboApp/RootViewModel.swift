import Foundation
import Observation
import SwiftUI
import CoreModel
import CoreData
import CoreAuth
import CoreSync

/// App-level state that outlives any one screen: the persisted theme mode (so the
/// whole app re-themes from Settings) and the current account (for the avatar /
/// account menu). Mirrors the bits `MainActivity` reads on Android.
@MainActor
@Observable
public final class RootViewModel {
    public private(set) var themeMode: ThemeMode = .system
    public private(set) var account: Account?
    /// The user's shareable friend code (loaded once signed in), else nil.
    public private(set) var friendCode: String?

    private let settingsRepository: SettingsRepository
    private let authRepository: AuthRepository
    private let sharingRepository: SharingRepository
    private var observations: [Task<Void, Never>] = []

    public init(settingsRepository: SettingsRepository, authRepository: AuthRepository, sharingRepository: SharingRepository) {
        self.settingsRepository = settingsRepository
        self.authRepository = authRepository
        self.sharingRepository = sharingRepository
    }

    public func start() {
        guard observations.isEmpty else { return }
        observations.append(Task { [weak self, settingsRepository] in
            for await settings in await settingsRepository.stream() {
                self?.themeMode = settings.themeMode
            }
        })
        observations.append(Task { [weak self, authRepository, sharingRepository] in
            for await state in await authRepository.state() {
                self?.account = state.account
                if state.account != nil {
                    self?.friendCode = await sharingRepository.friendCode().getOrNil()
                } else {
                    self?.friendCode = nil
                }
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
