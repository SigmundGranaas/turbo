import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'composite_search_service.dart' show kartverketLocationServiceProvider;
import 'kartverket_location_service.dart';

/// Reverse-geocoder provider. Backed by Kartverket today; future global
/// fallback (e.g. Nominatim) can be added behind the same surface.
///
/// Shares the same service instance (and HTTP client) as forward search so
/// connections are pooled and tests can override Kartverket once.
final reverseGeocoderProvider = Provider<KartverketLocationService>(
  (ref) => ref.watch(kartverketLocationServiceProvider),
);
