/// Public API for the path_recording feature.
///
/// Live GPS recording for hike-style tracks. The state machine accumulates
/// samples from a swappable [PositionSource] and produces a [RecordingResult]
/// that the UI can persist via the saved_paths feature.
library;

export 'data/position_source.dart'
    show PositionSource, positionSourceProvider;
export 'data/recording_notifier.dart'
    show recordingNotifierProvider, RecordingNotifier;
export 'models/recording_result.dart' show RecordingResult;
export 'models/recording_sample.dart' show RecordingSample;
export 'models/recording_state.dart' show RecordingState, RecordingStatus;
export 'widgets/recording_panel.dart' show RecordingPanel;
export 'widgets/recording_trace_layer.dart' show RecordingTraceLayer;
export 'widgets/start_recording_flow.dart' show startRecordingFlow;
