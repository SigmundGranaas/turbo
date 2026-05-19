import 'package:flutter/material.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import '../../models/vector_layer_source.dart';
import '../../widgets/trail_feature_sheet.dart';
import '../../widgets/trail_property_decoder.dart';

/// Vector source backed by the OpenStreetMap Overpass API.
///
/// Pulls all ways whose `highway` matches `path|footway|track|bridleway`
/// over the viewport — the OSM tag set that maps to Norwegian hiking
/// surfaces. Overpass returns its own JSON shape; the converter in
/// `core/util/overpass/overpass_to_geojson.dart` reshapes it to GeoJSON
/// so the existing vector pipeline handles the rest.
///
/// Attribution: © OpenStreetMap contributors (ODbL).
VectorLayerSource osmPathVectorSource() {
  return VectorLayerSource(
    id: 'osm_paths_vector',
    name: (c) => c.l10n.layerNameOsmPaths,
    color: const Color(0xFF8E44AD),
    // OSM tiles are short-lived caches in spirit but the *features*
    // here can be persisted — community edits rarely affect a tile
    // mid-trip, and offline value is high. The repo-level TTL still
    // expires us out after a week.
    persist: true,
    buildUri: ({
      required minLat,
      required minLon,
      required maxLat,
      required maxLon,
      maxFeatures,
    }) {
      final query = '[out:json][timeout:25];'
          '(way["highway"~"^(path|footway|track|bridleway)\$"]'
          '($minLat,$minLon,$maxLat,$maxLon););'
          'out geom ${maxFeatures ?? 400};';
      return Uri.https(
        'overpass-api.de',
        '/api/interpreter',
        {'data': query},
      );
    },
    sheetBuilder: (context, feature) => TrailFeatureSheet(
      feature: feature,
      subtypeLabel: context.l10n.layerNameOsmPaths,
      accent: const Color(0xFF8E44AD),
      decoded: TrailProperties.fromOsm(feature, context),
    ),
  );
}
