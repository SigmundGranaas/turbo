import 'package:flutter/material.dart';

/// Declarative description of an external GeoJSON / WFS layer.
class VectorLayerSource {
  /// Stable identifier (used as the cache namespace).
  final String id;

  /// Display name shown in feature sheets.
  final String Function(BuildContext) name;

  /// Builds the request URL for a given bbox. Implementers may decide whether
  /// to use a WFS GetFeature call, an Atom feed, or a static GeoJSON URL —
  /// the fetcher is agnostic and only parses GeoJSON.
  ///
  /// Bounds are passed in WGS84 (`minLon, minLat, maxLon, maxLat`).
  final Uri Function({
    required double minLat,
    required double minLon,
    required double maxLat,
    required double maxLon,
    int? maxFeatures,
  }) buildUri;

  /// Optional fixed query parameters forwarded as request headers (e.g.
  /// `User-Agent`). Per-request `User-Agent` injection lives in the fetcher,
  /// so this map is reserved for source-specific overrides.
  final Map<String, String>? headers;

  /// Render colour applied to lines / polygon strokes. Defaults to a neutral
  /// primary if not provided.
  final Color? color;

  /// Persist tiles into the on-device vector cache. Default true; pass false
  /// for short-TTL data like MetAlerts where staleness matters more than
  /// offline availability.
  final bool persist;

  /// When true, the fetcher returns an empty feature list without making a
  /// network call. Used to keep layer-picker / registry wiring intact for
  /// sources whose upstream is currently unavailable in a format we can
  /// parse (e.g. trail data — see [trailVectorSource]).
  final bool disabled;

  const VectorLayerSource({
    required this.id,
    required this.name,
    required this.buildUri,
    this.headers,
    this.color,
    this.persist = true,
    this.disabled = false,
  });
}
