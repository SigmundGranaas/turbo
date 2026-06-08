import WidgetKit
import SwiftUI

/// The widget extension's entry point. Hosts only the recording Live Activity
/// (no Home Screen widgets yet). The activity UI must live in an extension —
/// the app target can start/update/end the activity but can't render it.
@main
struct RecordingWidgetBundle: WidgetBundle {
    var body: some Widget {
        RecordingLiveActivity()
    }
}
