import 'package:latlong2/latlong.dart';

/// A single GPS fix consumed by the recorder. Decoupled from `geolocator`'s
/// `Position` so tests can supply samples without depending on the plugin.
class RecordingSample {
  final LatLng position;
  final double? elevation;
  final DateTime timestamp;
  final double? accuracyMeters;
  final double? speedMetersPerSecond;

  const RecordingSample({
    required this.position,
    required this.timestamp,
    this.elevation,
    this.accuracyMeters,
    this.speedMetersPerSecond,
  });
}
