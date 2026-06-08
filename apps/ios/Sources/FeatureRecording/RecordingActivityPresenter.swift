import Foundation

/// The seam the recording view model uses to drive a Live Activity, so the view
/// model itself stays free of ActivityKit (and host-testable on macOS). The real
/// implementation is `LiveActivityPresenter` (iOS only); tests use a spy and the
/// default is a no-op.
@MainActor
public protocol RecordingActivityPresenter {
    /// Start the Lock Screen / Dynamic Island activity for a new recording.
    func begin(title: String)
    /// Push the latest live stats.
    func update(distanceMeters: Double, elapsedSeconds: Int)
    /// Tear the activity down.
    func end()
}

/// Default presenter — does nothing (used where Live Activities are unavailable
/// or unwanted, e.g. the macOS host build and `-uitest`).
@MainActor
public struct NoRecordingActivityPresenter: RecordingActivityPresenter {
    public init() {}
    public func begin(title: String) {}
    public func update(distanceMeters: Double, elapsedSeconds: Int) {}
    public func end() {}
}

#if canImport(ActivityKit) && os(iOS)
@preconcurrency import ActivityKit
import CoreLiveActivity

/// Drives a real ActivityKit Live Activity for the active recording. No-ops
/// gracefully when the user has Live Activities disabled.
@MainActor
public final class LiveActivityPresenter: RecordingActivityPresenter {
    private var activity: Activity<RecordingActivityAttributes>?

    public init() {}

    public func begin(title: String) {
        guard ActivityAuthorizationInfo().areActivitiesEnabled, activity == nil else { return }
        let attributes = RecordingActivityAttributes(title: title)
        let state = RecordingActivityAttributes.ContentState(distanceMeters: 0, elapsedSeconds: 0)
        activity = try? Activity.request(attributes: attributes, content: .init(state: state, staleDate: nil))
    }

    public func update(distanceMeters: Double, elapsedSeconds: Int) {
        guard let activity else { return }
        let content = ActivityContent(
            state: RecordingActivityAttributes.ContentState(distanceMeters: distanceMeters, elapsedSeconds: elapsedSeconds),
            staleDate: nil
        )
        Task { await activity.update(content) }
    }

    public func end() {
        guard let activity else { return }
        self.activity = nil
        Task { await activity.end(nil, dismissalPolicy: .immediate) }
    }
}
#endif
