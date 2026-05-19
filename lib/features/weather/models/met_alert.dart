import 'package:latlong2/latlong.dart';

/// Severity tier reported by MET Norway's MetAlerts API. Maps to the
/// CAP/Awareness-Levels 2–4 colors used across European national met offices.
enum MetAlertLevel {
  /// Yellow / awareness level 2 — be aware, conditions may impact some.
  yellow,

  /// Orange / awareness level 3 — be prepared, severe conditions expected.
  orange,

  /// Red / awareness level 4 — take action, extreme conditions.
  red,
}

/// One severe-weather warning issued by MET Norway.
class MetAlert {
  final String id;
  final MetAlertLevel level;

  /// CAP event type (e.g. "rain", "wind", "snow", "ice", "avalanches").
  final String event;

  /// Localised headline / description, picked up from CAP `headline` or
  /// `description` when present.
  final String description;

  /// When the warning takes effect.
  final DateTime onset;

  /// When the warning expires.
  final DateTime expires;

  /// Polygon vertices describing the affected area. May be empty when the
  /// payload only carries a multi-polygon, in which case the alert is point-
  /// based and consumers should not try to render geometry.
  final List<LatLng> area;

  const MetAlert({
    required this.id,
    required this.level,
    required this.event,
    required this.description,
    required this.onset,
    required this.expires,
    required this.area,
  });

  static MetAlertLevel? parseLevel(String? raw) {
    if (raw == null) return null;
    final v = raw.toLowerCase().trim();
    // CAP awareness-level values: "2; yellow; Moderate", "3; orange; Severe",
    // "4; red; Extreme". Match on the colour name regardless of digit prefix.
    if (v.contains('yellow') || v.startsWith('2')) return MetAlertLevel.yellow;
    if (v.contains('orange') || v.startsWith('3')) return MetAlertLevel.orange;
    if (v.contains('red') || v.startsWith('4')) return MetAlertLevel.red;
    return null;
  }
}
