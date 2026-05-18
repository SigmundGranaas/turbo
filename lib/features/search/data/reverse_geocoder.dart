import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'kartverket_location_service.dart';

/// Reverse-geocoder provider. Backed by Kartverket today; future global
/// fallback (e.g. Nominatim) can be added behind the same surface.
final reverseGeocoderProvider = Provider<KartverketLocationService>(
  (ref) => KartverketLocationService(),
);
