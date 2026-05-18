import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:logging/logging.dart';
import 'package:turbo/core/location/follow_mode_state.dart';
import 'package:turbo/features/saved_paths/api.dart' show ElevationStats;
import 'package:turbo/features/settings/api.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

import '../models/recording_result.dart';
import '../models/recording_sample.dart';
import '../models/recording_state.dart';
import 'position_source.dart';

final _log = Logger('RecordingNotifier');

/// State machine driving a hike-recording session.
///
/// The notifier accumulates samples on every GPS fix, but emits new state
/// snapshots at most every [_emitInterval] so the HUD does not rebuild more
/// than a few times per second on slow devices. Callers that need
/// millisecond-fresh data should compute it from the snapshot directly.
class RecordingNotifier extends Notifier<RecordingState> {
  StreamSubscription<RecordingSample>? _subscription;
  Timer? _emitTimer;
  bool _wakelockHeld = false;

  static const Duration _emitInterval = Duration(milliseconds: 500);

  // Live accumulators. These are mutated as samples arrive; the public state
  // is rebuilt from them on the emit cadence.
  final List<LatLng> _points = [];
  final List<double?> _elevations = [];
  double _distance = 0;
  int _movingTimeMs = 0;
  DateTime? _startedAt;
  DateTime? _lastFixAt;
  DateTime? _lastSampleAt;
  static const Distance _distanceCalc = Distance();

  @override
  RecordingState build() {
    ref.onDispose(() {
      _subscription?.cancel();
      _emitTimer?.cancel();
      if (_wakelockHeld) {
        WakelockPlus.disable().ignore();
      }
    });
    return RecordingState.idle;
  }

  /// Begins a new session. If a previous session left accumulators populated
  /// (e.g. paused but never stopped) it is discarded.
  Future<void> start() async {
    if (state.status == RecordingStatus.recording) return;
    _resetBuffers();
    _startedAt = DateTime.now();

    final settings = ref.read(settingsProvider).value;
    final mode = settings?.gpsAccuracyMode ?? GpsAccuracyMode.high;
    final keepScreenOn = settings?.keepScreenOnWhileRecording ?? true;

    if (keepScreenOn) await _enableWakelock();

    // Starting a recording is the strongest possible "I want to see myself
    // on the map" signal — engage snap mode so the map tracks the user. If
    // they drag during the hike, follow goes to paused (per FollowModeNotifier);
    // tapping the location button resumes.
    ref.read(followModeProvider.notifier).enable();

    final source = ref.read(positionSourceProvider);
    _subscribe(source.stream(mode));
    _startEmitTimer();

    state = state.copyWith(
      status: RecordingStatus.recording,
      points: const [],
      elevations: const [],
      distanceMeters: 0,
      movingTimeSeconds: 0,
      ascent: 0,
      descent: 0,
      startedAt: _startedAt,
    );
  }

  void pause() {
    if (state.status != RecordingStatus.recording) return;
    _subscription?.cancel();
    _subscription = null;
    _emitTimer?.cancel();
    _emitTimer = null;
    state = state.copyWith(status: RecordingStatus.paused);
  }

  Future<void> resume() async {
    if (state.status != RecordingStatus.paused) return;
    final settings = ref.read(settingsProvider).value;
    final mode = settings?.gpsAccuracyMode ?? GpsAccuracyMode.high;
    // Reset the inter-sample timer so the pause gap is not counted as a
    // 10-minute "moving" segment when the next fix arrives.
    _lastSampleAt = null;
    final source = ref.read(positionSourceProvider);
    _subscribe(source.stream(mode));
    _startEmitTimer();
    state = state.copyWith(status: RecordingStatus.recording);
  }

  /// Ends the session and returns the captured data. The notifier resets
  /// to [RecordingState.idle] before returning, so the UI can immediately
  /// open `SavePathSheet` without observing stale state.
  Future<RecordingResult?> stop() async {
    if (state.status == RecordingStatus.idle) return null;
    await _teardown();

    if (_points.length < 2) {
      _resetBuffers();
      state = RecordingState.idle;
      return null;
    }

    final stats = ElevationStats.fromSamples(_elevations);
    final result = RecordingResult(
      points: List.unmodifiable(_points),
      elevations: List.unmodifiable(_elevations),
      distanceMeters: _distance,
      movingTimeSeconds: (_movingTimeMs / 1000).round(),
      ascent: stats.ascent,
      descent: stats.descent,
      recordedAt: _startedAt ?? DateTime.now(),
    );

    _resetBuffers();
    state = RecordingState.idle;
    return result;
  }

  /// Throws away the in-flight session without producing a result.
  Future<void> discard() async {
    await _teardown();
    _resetBuffers();
    state = RecordingState.idle;
  }

  void _subscribe(Stream<RecordingSample> stream) {
    _subscription = stream.listen(
      _onSample,
      onError: (Object e) => _log.warning('Recording stream error: $e'),
    );
  }

  /// Per-mode horizontal accuracy threshold (meters). Samples reporting a
  /// worse fix are dropped — these are typically the cold-start handful of
  /// readings where the chip is still picking up satellites and the track
  /// would otherwise zig-zag around the actual start location.
  double _maxAcceptableAccuracy(GpsAccuracyMode mode) {
    return switch (mode) {
      GpsAccuracyMode.high => 25.0,
      GpsAccuracyMode.balanced => 50.0,
      GpsAccuracyMode.batterySaver => 100.0,
    };
  }

  void _onSample(RecordingSample sample) {
    // Drop low-accuracy fixes. Only filter when the source actually reports
    // accuracy (real geolocator samples do; tests intentionally don't, so
    // synthetic fixtures stay deterministic).
    final accuracy = sample.accuracyMeters;
    if (accuracy != null) {
      final mode = ref.read(settingsProvider).value?.gpsAccuracyMode ??
          GpsAccuracyMode.high;
      if (accuracy > _maxAcceptableAccuracy(mode)) {
        _log.fine('Dropping low-accuracy sample (±${accuracy.toStringAsFixed(1)} m)');
        return;
      }
      // Reject implausible jumps — only when we have an accuracy report so
      // we know this is a real device sample, not a test fixture. > 50 m/s
      // (180 km/h) is past anything hike/bike/ski use cases involve.
      if (_points.isNotEmpty && _lastSampleAt != null) {
        final segment = _distanceCalc.distance(_points.last, sample.position);
        final dtMs = sample.timestamp.difference(_lastSampleAt!).inMilliseconds;
        if (dtMs > 0 && segment / (dtMs / 1000.0) > 50) {
          _log.fine(
              'Dropping jump sample (${segment.toStringAsFixed(0)} m in ${dtMs}ms)');
          return;
        }
      }
    }

    final now = sample.timestamp;
    _lastFixAt = now;

    if (_points.isNotEmpty) {
      final last = _points.last;
      final segment = _distanceCalc.distance(last, sample.position);
      _distance += segment;
      if (_lastSampleAt != null) {
        final dtMs = now.difference(_lastSampleAt!).inMilliseconds;
        // Anything slower than 0.3 m/s (≈ 1 km/h) counts as "stopped"; this
        // keeps coffee breaks out of moving time without dropping legitimate
        // slow walking.
        final speed = dtMs > 0 ? segment / (dtMs / 1000.0) : 0.0;
        if (speed > 0.3 && dtMs < 30_000) {
          _movingTimeMs += dtMs;
        }
      }
    }
    _points.add(sample.position);
    _elevations.add(sample.elevation);
    _lastSampleAt = now;
  }

  void _startEmitTimer() {
    _emitTimer?.cancel();
    _emitTimer = Timer.periodic(_emitInterval, (_) => _emitSnapshot());
  }

  void _emitSnapshot() {
    if (state.status != RecordingStatus.recording) return;
    final stats = ElevationStats.fromSamples(_elevations);
    state = state.copyWith(
      points: List.unmodifiable(_points),
      elevations: List.unmodifiable(_elevations),
      distanceMeters: _distance,
      movingTimeSeconds: (_movingTimeMs / 1000).round(),
      ascent: stats.ascent,
      descent: stats.descent,
      lastFixAt: _lastFixAt,
    );
  }

  Future<void> _teardown() async {
    await _subscription?.cancel();
    _subscription = null;
    _emitTimer?.cancel();
    _emitTimer = null;
    if (_wakelockHeld) {
      await WakelockPlus.disable();
      _wakelockHeld = false;
    }
  }

  void _resetBuffers() {
    _points.clear();
    _elevations.clear();
    _distance = 0;
    _movingTimeMs = 0;
    _startedAt = null;
    _lastFixAt = null;
    _lastSampleAt = null;
  }

  Future<void> _enableWakelock() async {
    try {
      await WakelockPlus.enable();
      _wakelockHeld = true;
    } catch (e) {
      _log.warning('Failed to enable wakelock: $e');
    }
  }
}

/// Public provider. Intentionally NOT autoDispose: navigating away from the
/// map screen must not kill an active recording.
final recordingNotifierProvider =
    NotifierProvider<RecordingNotifier, RecordingState>(RecordingNotifier.new);
