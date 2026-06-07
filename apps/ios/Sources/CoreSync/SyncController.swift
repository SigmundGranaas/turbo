import Foundation
import CoreData
import CoreAuth

/// Orchestrates a sync pass, gated like Android's `SyncController`: only runs when
/// signed in *and* cloud sync is enabled in settings. Call ``syncNow()`` when the
/// app comes to the foreground (mirrors `MainActivity.onResume`).
public actor SyncController {
    private let engine: MarkerSyncEngine
    private let auth: AuthRepository
    private let settings: SettingsRepository
    private var inFlight = false

    public init(engine: MarkerSyncEngine, auth: AuthRepository, settings: SettingsRepository) {
        self.engine = engine
        self.auth = auth
        self.settings = settings
    }

    /// Pull + push once. No-op when signed out, when sync is disabled, or when a
    /// pass is already running. Errors are swallowed (sync is best-effort).
    public func syncNow() async {
        guard !inFlight else { return }
        guard case .signedIn = await auth.current() else { return }
        guard await settings.current().cloudSyncEnabled else { return }
        inFlight = true
        defer { inFlight = false }
        try? await engine.sync()
    }
}
