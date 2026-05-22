import 'package:flutter/material.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import '../../models/vector_layer_source.dart';
import '../../widgets/trail_feature_sheet.dart';

/// One Nasjonal turbase trail subtype rendered as a vector layer.
enum TrailSubtype {
  /// `Fotrute` — marked footpaths / hiking trails (DNT-red on the map).
  foot,

  /// `Skiløype` — prepared ski tracks (blue on the map).
  ski,

  /// `Sykkelrute` — designated bike routes (green on the map).
  bike,

  /// `AnnenRute` — everything else (horse, paddling, ...).
  other,
}

/// Build a vector source for a single trail subtype.
///
/// Targets Geonorge's canonical WFS for the Turrutebasen dataset
/// (`wfs.turogfriluftsruter`). The endpoint only emits GML 3.2.1
/// (`application/json` is refused with HTTP 400), so the source asks
/// for `text/xml; subtype=gml/3.2.1`; [VectorLayerFetcher] then
/// detects the XML content-type and runs the body through
/// [GmlToGeoJson] before the existing GeoJSON parser.
///
/// SRS is `urn:ogc:def:crs:EPSG::4326`, which under the urn form means
/// **lat,lon** in the `gml:posList`. The converter normalises that to
/// GeoJSON's `[lon, lat]`.
VectorLayerSource trailVectorSource(TrailSubtype subtype) {
  final spec = _specs[subtype]!;
  return VectorLayerSource(
    id: 'trails_${spec.idSuffix}_vector',
    name: spec.name,
    color: spec.color,
    buildUri: ({
      required minLat,
      required minLon,
      required maxLat,
      required maxLon,
      maxFeatures,
    }) {
      return Uri.https(
          'wfs.geonorge.no', '/skwms1/wfs.turogfriluftsruter', {
        'SERVICE': 'WFS',
        'VERSION': '2.0.0',
        'REQUEST': 'GetFeature',
        'TYPENAMES': spec.typeName,
        'OUTPUTFORMAT': 'text/xml; subtype=gml/3.2.1',
        'SRSNAME': 'urn:ogc:def:crs:EPSG::4326',
        'BBOX':
            '$minLat,$minLon,$maxLat,$maxLon,urn:ogc:def:crs:EPSG::4326',
        'COUNT': '${maxFeatures ?? 300}',
      });
    },
    sheetBuilder: (context, feature) => TrailFeatureSheet(
      feature: feature,
      subtypeLabel: spec.name(context),
      accent: spec.color,
    ),
  );
}

class _TrailSpec {
  final String idSuffix;
  final String typeName;
  final Color color;
  final String Function(BuildContext) name;
  const _TrailSpec({
    required this.idSuffix,
    required this.typeName,
    required this.color,
    required this.name,
  });
}

// Canonical (capitalised + namespaced) feature-type names as advertised
// by the wfs.turogfriluftsruter GetCapabilities. The lowercase variants
// (fotrute / skiloype / sykkelrute / andreruter) do not exist on any
// Geonorge WFS host.
final Map<TrailSubtype, _TrailSpec> _specs = {
  TrailSubtype.foot: _TrailSpec(
    idSuffix: 'foot',
    typeName: 'app:Fotrute',
    color: const Color(0xFFE63946),
    name: (c) => c.l10n.layerNameTrailsFoot,
  ),
  TrailSubtype.ski: _TrailSpec(
    idSuffix: 'ski',
    typeName: 'app:Skiløype',
    color: const Color(0xFF1976D2),
    name: (c) => c.l10n.layerNameTrailsSki,
  ),
  TrailSubtype.bike: _TrailSpec(
    idSuffix: 'bike',
    typeName: 'app:Sykkelrute',
    color: const Color(0xFF388E3C),
    name: (c) => c.l10n.layerNameTrailsBike,
  ),
  TrailSubtype.other: _TrailSpec(
    idSuffix: 'other',
    typeName: 'app:AnnenRute',
    color: const Color(0xFFFB8C00),
    name: (c) => c.l10n.layerNameTrailsOther,
  ),
};

/// Maps the registry overlay id (toggled via the layer picker) to the
/// matching vector source. Trail subtype IDs are aligned across the WMS
/// overlay and the vector source: `trails_<subtype>` activates both.
const Map<String, TrailSubtype> trailOverlayIdToSubtype = {
  'trails_foot': TrailSubtype.foot,
  'trails_ski': TrailSubtype.ski,
  'trails_bike': TrailSubtype.bike,
  'trails_other': TrailSubtype.other,
};

// Trail feature presentation lives in `widgets/trail_feature_sheet.dart`
// + `widgets/trail_property_decoder.dart`. The trail source wires the
// `TrailFeatureSheet` via `VectorLayerSource.sheetBuilder` above.
