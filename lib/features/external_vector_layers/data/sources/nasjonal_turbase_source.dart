import 'package:flutter/material.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import '../../models/vector_layer_source.dart';

/// Source descriptor for the Geonorge / Kartverket "Nasjonal turbase"
/// (national trail register) served as a WFS feed.
///
/// We hit the WFS GetFeature endpoint with a `bbox=` filter; the response is
/// a GeoJSON FeatureCollection of LineString geometries representing the
/// marked trail network. Property names are Norwegian (e.g. `navn`,
/// `rutenummer`).
VectorLayerSource nasjonalTurbaseVectorSource() {
  return VectorLayerSource(
    id: 'nasjonal_turbase_vector',
    name: (c) => c.l10n.layerNameTrails,
    color: const Color(0xFFD32F2F),
    buildUri: ({
      required minLat,
      required minLon,
      required maxLat,
      required maxLon,
      maxFeatures,
    }) {
      return Uri.https('wfs.geonorge.no', '/skwms1/wfs.friluftsruter2', {
        'SERVICE': 'WFS',
        'VERSION': '2.0.0',
        'REQUEST': 'GetFeature',
        'TYPENAMES': 'app:Fotrute,app:Skiloype,app:Annenrute',
        'OUTPUTFORMAT': 'application/json',
        'SRSNAME': 'EPSG:4326',
        // WFS expects bbox as minLat,minLon,maxLat,maxLon when SRSNAME is in
        // EPSG:4326 (lat,lon ordering, per WFS 2.0).
        'BBOX': '$minLat,$minLon,$maxLat,$maxLon,EPSG:4326',
        'COUNT': '${maxFeatures ?? 300}',
      });
    },
  );
}

/// Property keys we surface in the trail-info sheet. Anything else in the
/// feature payload is hidden so the sheet stays useful at a glance.
const trailPropertyKeys = <String>[
  'navn',
  'rutenummer',
  'merkemetode',
  'vanskelighet',
  'rutebredde',
  'belegg',
  'lengde',
  'sesong',
];

/// Maps Norwegian property keys onto localised labels for [showVectorFeatureSheet].
Map<String, String> trailPropertyLabels(BuildContext context) {
  final l10n = context.l10n;
  return {
    'navn': l10n.name,
    'rutenummer': l10n.trailRouteNumberLabel,
    'merkemetode': l10n.trailMarkingLabel,
    'vanskelighet': l10n.trailDifficultyLabel,
    'rutebredde': l10n.trailLengthLabel,
    'lengde': l10n.trailLengthLabel,
  };
}
