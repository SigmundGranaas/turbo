import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:http/http.dart' as http;

import 'location_service.dart';

/// Disabled trail search source.
///
/// Why this is a no-op: PR #75 originally targeted
/// `wfs.geonorge.no/skwms1/wfs.friluftsruter2` with a CQL `LIKE` filter on
/// `navn`. Two facts make that combination unrunnable:
///
///   1. The `wfs.friluftsruter2` URL does not exist — the server responds
///      with `UKJENT APPLIKASJON` (unknown application). The canonical
///      WFS for the Turrutebasen dataset lives at
///      `wfs.geonorge.no/skwms1/wfs.turogfriluftsruter`.
///   2. That canonical WFS rejects `application/json` output
///      ("not configured to handle the output/input format
///      'application/json'") and silently ignores `CQL_FILTER`, so even
///      pointed at the right host, a name search is structurally
///      impossible — every request returns the first N trails regardless
///      of input.
///
/// Trail names that Kartverket considers toponyms (peaks, ridges, water
/// the trail crosses) are still surfaced by the Stedsnavn `/navn` forward
/// search and the composite search service, so users searching for
/// "Besseggen" still get a Besseggen hit (as feature kind `Egg`).
///
/// The WMS raster overlay at `wms.geonorge.no/skwms1/wms.friluftsruter2`
/// continues to render trails on the map.
class TrailSearchService extends LocationService {
  // The unused client parameter is kept so existing tests that pass a
  // MockClient (and existing call sites) compile unchanged.
  // ignore: unused_field
  final http.Client? _client;

  TrailSearchService({http.Client? client}) : _client = client;

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    return const [];
  }
}

final trailSearchServiceProvider =
    Provider<TrailSearchService>((_) => TrailSearchService());
