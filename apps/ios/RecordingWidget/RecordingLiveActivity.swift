import ActivityKit
import WidgetKit
import SwiftUI
import CoreLiveActivity

/// Renders the active-recording Live Activity on the Lock Screen and in the
/// Dynamic Island, driven by `RecordingActivityAttributes` updates pushed from
/// the app's `LiveActivityPresenter`. The iOS analogue of the Android recording
/// foreground-service notification.
struct RecordingLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: RecordingActivityAttributes.self) { context in
            // Lock Screen / banner presentation.
            HStack(spacing: 14) {
                Image(systemName: "figure.hiking")
                    .font(.title2).foregroundStyle(.green)
                VStack(alignment: .leading, spacing: 2) {
                    Text(context.attributes.title)
                        .font(.headline)
                    Text(context.state.formattedDistance)
                        .font(.subheadline).foregroundStyle(.secondary)
                }
                Spacer()
                Text(context.state.formattedElapsed)
                    .font(.system(.title2, design: .rounded)).monospacedDigit().bold()
            }
            .padding()
            .activityBackgroundTint(Color.black.opacity(0.5))
            .activitySystemActionForegroundColor(.white)
        } dynamicIsland: { context in
            DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    Label(context.state.formattedDistance, systemImage: "ruler")
                        .font(.headline)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    Label(context.state.formattedElapsed, systemImage: "clock")
                        .font(.system(.headline, design: .rounded)).monospacedDigit()
                }
                DynamicIslandExpandedRegion(.bottom) {
                    Text(context.attributes.title).font(.caption).foregroundStyle(.secondary)
                }
            } compactLeading: {
                Image(systemName: "figure.hiking").foregroundStyle(.green)
            } compactTrailing: {
                Text(context.state.formattedElapsed).monospacedDigit()
            } minimal: {
                Image(systemName: "figure.hiking").foregroundStyle(.green)
            }
        }
    }
}
