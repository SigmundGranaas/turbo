import Foundation
import Observation

/// Observable cloud-sync status, so the UI can show whether the user's data
/// actually reached the server (it was previously fire-and-forget with errors
/// swallowed). The ``SyncController`` reports into this; screens observe it.
@MainActor
@Observable
public final class SyncStatus {
    public enum Phase: Equatable, Sendable {
        case idle
        case syncing
        case synced
        case failed(String)
    }

    public private(set) var phase: Phase = .idle
    /// When the last successful pass completed, for a "synced N ago" label.
    public private(set) var lastSyncedAt: Date?

    public init() {}

    public func begin() { phase = .syncing }

    public func succeed(at date: Date) {
        phase = .synced
        lastSyncedAt = date
    }

    public func fail(_ message: String) { phase = .failed(message) }

    /// A short, human label for the current state (relative to `now`).
    public func summary(now: Date = Date()) -> String {
        switch phase {
        case .syncing: return "Syncing…"
        case .failed: return "Sync failed"
        case .idle where lastSyncedAt == nil: return "Not synced yet"
        case .idle, .synced:
            guard let last = lastSyncedAt else { return "Not synced yet" }
            return "Synced \(Self.relative(from: last, to: now))"
        }
    }

    public var isFailed: Bool { if case .failed = phase { return true } else { return false } }

    public var failureMessage: String? { if case .failed(let m) = phase { return m } else { return nil } }

    static func relative(from: Date, to now: Date) -> String {
        let seconds = Int(now.timeIntervalSince(from))
        switch seconds {
        case ..<0, 0..<60: return "just now"
        case 60..<3600: return "\(seconds / 60)m ago"
        case 3600..<86_400: return "\(seconds / 3600)h ago"
        default: return "\(seconds / 86_400)d ago"
        }
    }
}
