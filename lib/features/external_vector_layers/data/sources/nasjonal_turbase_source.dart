import 'package:flutter/material.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import '../../models/vector_layer_source.dart';

/// One Nasjonal turbase trail subtype rendered as a vector layer.
enum TrailSubtype {
  /// `fotrute` — marked footpaths / hiking trails (DNT-red on the map).
  foot,

  /// `skiloype` — prepared ski tracks (blue on the map).
  ski,

  /// `sykkelrute` — designated bike routes (green on the map).
  bike,

  /// `andreruter` — everything else (horse, paddling, ...).
  other,
}

/// Build a vector source for a single trail subtype.
///
/// **Currently disabled.** PR #75 originally pointed this at
/// `wfs.geonorge.no/skwms1/wfs.friluftsruter2`, which doesn't exist (the
/// server replies `UKJENT APPLIKASJON`). The canonical WFS for the
/// Turrutebasen dataset is `wfs.geonorge.no/skwms1/wfs.turogfriluftsruter`,
/// but that endpoint refuses `application/json` output and only emits
/// GML 3.2.1 — which [VectorLayerFetcher] doesn't parse. Adding a GML
/// parser is out of scope for the immediate fix, so [disabled] is set
/// and the fetcher returns no features without making a request.
///
/// The trail data is still shown on the map via the WMS raster overlay
/// at `wms.geonorge.no/skwms1/wms.friluftsruter2` (see
/// `nasjonal_turbase_overlay.dart`) — that path is unaffected. The
/// canonical URL is still built so debug logging and tests can verify
/// the intended destination.
VectorLayerSource trailVectorSource(TrailSubtype subtype) {
  final spec = _specs[subtype]!;
  return VectorLayerSource(
    id: 'trails_${spec.idSuffix}_vector',
    name: spec.name,
    color: spec.color,
    disabled: true,
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
// by the WFS GetCapabilities for wfs.turogfriluftsruter.
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
