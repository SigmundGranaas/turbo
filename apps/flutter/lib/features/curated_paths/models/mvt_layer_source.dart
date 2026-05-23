import 'package:flutter/material.dart';

import 'package:turbo/features/external_vector_layers/api.dart';

/// Declarative description of one curated MVT-served layer exposed by
/// the Turbo tileserver (`/v1/{resource}/tiles/{z}/{x}/{y}.mvt`).
///
/// Sibling to `VectorLayerSource` from `external_vector_layers/`. We
/// keep them separate because the transport (binary protobuf) and the
/// cache shape (raw bytes vs JSON) diverge enough that overloading the
/// existing model would muddy it.
typedef MvtFeatureSheetBuilder = Widget Function(
  BuildContext context,
  VectorFeature feature,
);

class MvtLayerSource {
  /// Stable identifier matching the tileserver resource slug
  /// (`hiking-trails`, `ski-tracks`, `forest-roads`, `cycling-routes`)
  /// and used as the cache namespace.
  final String id;

  /// Display name shown in feature sheets.
  final String Function(BuildContext) name;

  /// URL template with `{z}/{x}/{y}` placeholders. Built from the
  /// tileserver's `tiles_url_template` field returned by `/v1/catalog`,
  /// or hard-wired per environment.
  final String tilesUrlTemplate;

  /// URL pattern with a single `{id}` placeholder, used by tap-to-sheet
  /// to fetch full feature metadata from the GeoJSON detail endpoint.
  final String geojsonDetailUrlTemplate;

  /// Render colour applied to lines and polygon strokes.
  final Color color;

  /// Bounds outside which the layer is hidden (avoids fetching tiles
  /// at unreasonable zooms).
  final double minZoom;
  final double maxZoom;

  /// Persist tiles into the on-device MVT cache. Default true.
  final bool persist;

  /// Custom feature-detail sheet body. When null, the generic
  /// key/value sheet is used.
  final MvtFeatureSheetBuilder? sheetBuilder;

  /// Forwarded as HTTP headers (e.g. tenant tokens). Auth tokens are
  /// added by the dio interceptor — leave that out of here.
  final Map<String, String>? headers;

  /// Attribution string surfaced in the layer settings sheet. The
  /// tileserver already embeds per-feature attribution; this is the
  /// fallback shown when no feature is selected.
  final String attribution;

  const MvtLayerSource({
    required this.id,
    required this.name,
    required this.tilesUrlTemplate,
    required this.geojsonDetailUrlTemplate,
    required this.color,
    required this.attribution,
    this.minZoom = 8,
    this.maxZoom = 18,
    this.persist = true,
    this.sheetBuilder,
    this.headers,
  });

  Uri tileUri(int z, int x, int y) {
    final filled = tilesUrlTemplate
        .replaceAll('{z}', '$z')
        .replaceAll('{x}', '$x')
        .replaceAll('{y}', '$y');
    return Uri.parse(filled);
  }

  Uri detailUri(String featureId) {
    final filled = geojsonDetailUrlTemplate.replaceAll('{id}', featureId);
    return Uri.parse(filled);
  }
}
