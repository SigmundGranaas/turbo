import 'package:flutter/material.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import '../../models/vector_layer_source.dart';
import '../../widgets/trail_feature_sheet.dart';
import '../../widgets/trail_property_decoder.dart';

/// Vector source for Kartverket's FKB Traktorveg+Sti dataset — the
/// tractor roads and footpaths that Norgeskart bakes into its basemap.
///
/// Endpoint nuances (all observed against the live GetCapabilities):
///   - The service is published under the `wms.geonorge.no` host
///     (despite being a true WFS), at `wms.traktorveg_skogsbilveger`.
///     The `wfs.geonorge.no` mirror is configured but returns HTTP 500
///     for GetFeature today; the WMS host is the one that actually
///     responds with GML.
///   - Feature types are `ms:traktorveg_sti` (combined tractor roads +
///     paths) and `ms:skogsbilveg` (forest roads). We pull both and
///     differentiate via the `typeveg` property in the sheet.
///   - SRS is `urn:ogc:def:crs:EPSG::4326` so the BBOX axis order is
///     lat,lon — same as Turrutebasen.
///
/// Attribution: © Kartverket (FKB-Traktorveg+Sti).
VectorLayerSource n50StiVectorSource() {
  return VectorLayerSource(
    id: 'n50_sti',
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
        'wms.geonorge.no',
        '/skwms1/wms.traktorveg_skogsbilveger',
        {
          'SERVICE': 'WFS',
          'VERSION': '2.0.0',
          'REQUEST': 'GetFeature',
          'TYPENAMES': 'ms:traktorveg_sti,ms:skogsbilveg',
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
