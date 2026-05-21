import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/core/location/gps_accuracy_mode.dart';

import '../models/recording_sample.dart';
import 'geolocator_position_source.dart';

/// Abstract seam between the recorder and the OS location stream.
///
/// Production wires this to `geolocator`. Tests override it with a fake that
/// pushes [RecordingSample]s through a controlled [StreamController].
abstract class PositionSource {
  /// Begins emitting samples. Returns a broadcast stream — multiple
  /// subscribers should not break the source. Implementations must request
  /// any required permissions before the first sample.
  Stream<RecordingSample> stream(GpsAccuracyMode mode);

  /// Tears down the underlying stream subscription. Idempotent.
  Future<void> dispose();
}

/// Bound to the geolocator-backed source by default. Override in tests with
/// `positionSourceProvider.overrideWithValue(_FakeSource())`.
final positionSourceProvider = Provider<PositionSource>((ref) {
  final source = GeolocatorPositionSource();
  ref.onDispose(source.dispose);
  return source;
});
