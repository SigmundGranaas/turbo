import 'package:latlong2/latlong.dart';

/// A trip's full detail: the route polyline to animate plus the metadata shown
/// in the info sheet. Built from a fully-fetched Nasjonal Turbase `Tur`.
class NtbRoute {
  final String id;
  final String title;

  /// Ordered route geometry (WGS84). Empty when the document had no usable
  /// line geometry — the sheet still shows, but there is nothing to reveal.
  final List<LatLng> points;

  final String? description;

  /// Route length in metres, if the document provided `distanse`.
  final double? distanceMeters;

  /// Difficulty grade (`gradering`), e.g. "Enkel" / "Middels" / "Krevende".
  final String? grade;

  final String? imageUrl;
  final String? utUrl;

  const NtbRoute({
    required this.id,
    required this.title,
    required this.points,
    this.description,
    this.distanceMeters,
    this.grade,
    this.imageUrl,
    this.utUrl,
  });

  bool get hasGeometry => points.length >= 2;
}
