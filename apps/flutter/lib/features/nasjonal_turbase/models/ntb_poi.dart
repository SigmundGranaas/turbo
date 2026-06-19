import 'package:latlong2/latlong.dart';

/// What kind of Nasjonal Turbase object a marker represents. Drives the pin
/// glyph/colour and whether tapping it triggers the animated route reveal.
enum NtbPoiType {
  /// A `Sted` tagged `Hytte` — a cabin.
  cabin,

  /// A `Tur` — a trip suggestion with a route geometry to reveal.
  trip,

  /// Any other `Sted` (turmål, viewpoint, parking, …).
  place,
}

/// A lightweight marker projected from a Nasjonal Turbase document. Holds only
/// what the map pin and info sheet need; the full route geometry for a [trip]
/// is fetched lazily (see `NtbClient.fetchRoute`).
class NtbPoi {
  final String id;
  final NtbPoiType type;
  final String title;
  final LatLng position;
  final String? summary;
  final String? imageUrl;

  /// Best outbound link to ut.no (prefers the document's own `lenker`, else a
  /// constructed `ut.no/{type}/{id}` URL). May be `null` if neither is known.
  final String? utUrl;

  const NtbPoi({
    required this.id,
    required this.type,
    required this.title,
    required this.position,
    this.summary,
    this.imageUrl,
    this.utUrl,
  });

  /// Trips carry a route polyline that can be animated on selection.
  bool get hasRoute => type == NtbPoiType.trip;
}
