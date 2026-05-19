import 'package:flutter/material.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import '../../models/vector_layer_source.dart';
import '../../widgets/trail_feature_sheet.dart';
import '../../widgets/trail_property_decoder.dart';

/// Vector source for Kartverket's N50 `Sti` and `TraktorvegSti` features
/// — the paths that Norgeskart bakes into its raster basemap. Unlike
/// Turrutebasen this is *map data*, not curated route data: individual
/// segments are rarely named and there are no difficulty/season fields.
/// Use this when the user wants to see "everything Norgeskart shows".
///
/// Endpoint: the Geonorge `wfs.fkb-traktorveg-sti` service. Output is
/// GML 3.2.1 (same as Turrutebasen) so the existing converter handles
/// the response.
///
/// Attribution: © Kartverket (N50 Kartdata).
VectorLayerSource n50StiVectorSource() {
  return VectorLayerSource(
    id: 'n50_sti_vector',
    name: (c) => c.l10n.layerNameN50Sti,
    color: const Color(0xFF6D4C41),
    persist: true,
    buildUri: ({
      required minLat,
      required minLon,
      required maxLat,
      required maxLon,
      maxFeatures,
    }) {
      return Uri.https(
        'wfs.geonorge.no',
        '/skwms1/wfs.fkb-traktorveg-sti',
        {
          'SERVICE': 'WFS',
          'VERSION': '2.0.0',
          'REQUEST': 'GetFeature',
          'TYPENAMES': 'app:Sti,app:TraktorvegSti',
          'OUTPUTFORMAT': 'text/xml; subtype=gml/3.2.1',
          'SRSNAME': 'urn:ogc:def:crs:EPSG::4326',
          'BBOX':
              '$minLat,$minLon,$maxLat,$maxLon,urn:ogc:def:crs:EPSG::4326',
          'COUNT': '${maxFeatures ?? 400}',
        },
      );
    },
    sheetBuilder: (context, feature) => TrailFeatureSheet(
      feature: feature,
      subtypeLabel: context.l10n.layerNameN50Sti,
      accent: const Color(0xFF6D4C41),
      decoded: TrailProperties.fromN50Sti(feature, context),
    ),
  );
}
