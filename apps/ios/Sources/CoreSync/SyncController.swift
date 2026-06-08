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
    private let status: SyncStatus?
    private let now: @Sendable () -> Date
    private var inFlight = false

    public init(units: [any SyncUnit], auth: AuthRepository, settings: SettingsRepository,
                status: SyncStatus? = nil, now: @escaping @Sendable () -> Date = { Date() }) {
        self.units = units
        self.auth = auth
        self.settings = settings
        self.status = status
        self.now = now
    }

    /// Pull + push every entity once. No-op when signed out, sync disabled, or a
    /// pass is already running. Per-unit errors are counted and reported via
    /// ``SyncStatus`` so a failed pass is visible (and retryable) rather than silent.
    public func syncNow() async {
        guard !inFlight else { return }
        guard case .signedIn = await auth.current() else { return }
        guard await settings.current().cloudSyncEnabled else { return }
        inFlight = true
        defer { inFlight = false }

        await report { $0.begin() }
        var failures = 0
        for unit in units {
            do { try await unit.sync() } catch { failures += 1 }
        }
        let stamp = now()
        if failures == 0 {
            await report { $0.succeed(at: stamp) }
        } else {
            let noun = failures == 1 ? "item type" : "item types"
            await report { $0.fail("\(failures) \(noun) didn't sync. Tap to retry.") }
        }
    }

    private func report(_ update: @escaping @MainActor (SyncStatus) -> Void) async {
        guard let status else { return }
        await MainActor.run { update(status) }
    }
}
