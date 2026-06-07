import Foundation
import CoreData
import CoreAuth

/// Orchestrates a sync pass across all entity engines, gated like Android's
/// `SyncController`: only runs when signed in *and* cloud sync is enabled. Call
/// ``syncNow()`` when the app foregrounds (mirrors `MainActivity.onResume`).
public actor SyncController {
    private let units: [any SyncUnit]
    private let auth: AuthRepository
    private let settings: SettingsRepository
    private var inFlight = false

    public init(units: [any SyncUnit], auth: AuthRepository, settings: SettingsRepository) {
        self.units = units
        self.auth = auth
        self.settings = settings
    }

    /// Pull + push every entity once. No-op when signed out, sync disabled, or a
    /// pass is already running. Per-unit errors are swallowed (best-effort).
    public func syncNow() async {
        guard !inFlight else { return }
        guard case .signedIn = await auth.current() else { return }
        guard await settings.current().cloudSyncEnabled else { return }
        inFlight = true
        defer { inFlight = false }
        for unit in units {
            try? await unit.sync()
        }
    }
}
