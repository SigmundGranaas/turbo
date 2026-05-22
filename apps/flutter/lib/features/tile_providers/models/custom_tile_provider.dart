import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';

/// Which on-the-wire format the user-supplied URL talks. Persisted alongside
/// the provider so the registry can render either kind correctly.
enum CustomUrlKind {
  /// XYZ / TMS tile servers — `{z}/{x}/{y}` substitution per tile.
  xyz,

  /// OGC WMS GetMap — server takes a bbox + width + height and returns a
  /// rendered image. The URL has `{bbox}/{width}/{height}` placeholders and
  /// `service=WMS` / `request=GetMap` query parameters; flutter_map's
  /// [WMSTileLayerOptions] builds the per-tile URL itself.
  wms;

  static CustomUrlKind fromName(String? name) =>
      name == 'wms' ? CustomUrlKind.wms : CustomUrlKind.xyz;

  /// Infers the kind from a raw URL template. `{bbox}` (lowercase as
  /// flutter_map's WMS layer uses it) — or `service=wms` anywhere in the
  /// query — signals WMS; otherwise XYZ.
  static CustomUrlKind detect(String template) {
    final lower = template.toLowerCase();
    if (lower.contains('{bbox}') || lower.contains('service=wms')) {
      return CustomUrlKind.wms;
    }
    return CustomUrlKind.xyz;
  }
}

/// A user-defined tile source. Persisted via SharedPreferences as JSON.
/// Distinct from the built-in TileProviderConfig subclasses because the
/// data is provided at runtime by the user, not at compile time.
@immutable
class CustomTileProvider {
  /// Stable id with a `custom_` prefix so it can never collide with a
  /// built-in provider id.
  final String id;
  final String displayName;
  final String urlTemplate;
  final TileProviderCategory category;
  final CustomUrlKind urlKind;
  final double minZoom;
  final double maxZoom;

  const CustomTileProvider({
    required this.id,
    required this.displayName,
    required this.urlTemplate,
    required this.category,
    this.urlKind = CustomUrlKind.xyz,
    this.minZoom = 1,
    this.maxZoom = 19,
  });

  Map<String, Object?> toJson() => {
        'id': id,
        'displayName': displayName,
        'urlTemplate': urlTemplate,
        'category': category.name,
        'urlKind': urlKind.name,
        'minZoom': minZoom,
        'maxZoom': maxZoom,
      };

  factory CustomTileProvider.fromJson(Map<String, Object?> json) {
    return CustomTileProvider(
      id: json['id'] as String,
      displayName: json['displayName'] as String,
      urlTemplate: json['urlTemplate'] as String,
      category: TileProviderCategory.values
          .firstWhere((c) => c.name == json['category'],
              orElse: () => TileProviderCategory.global),
      urlKind: CustomUrlKind.fromName(json['urlKind'] as String?),
      minZoom: (json['minZoom'] as num?)?.toDouble() ?? 1,
      maxZoom: (json['maxZoom'] as num?)?.toDouble() ?? 19,
    );
  }

  static String encodeList(List<CustomTileProvider> list) =>
      jsonEncode(list.map((e) => e.toJson()).toList());

  static List<CustomTileProvider> decodeList(String? raw) {
    if (raw == null || raw.isEmpty) return const [];
    final decoded = jsonDecode(raw);
    if (decoded is! List) return const [];
    return decoded
        .whereType<Map>()
        .map((m) =>
            CustomTileProvider.fromJson(m.cast<String, Object?>()))
        .toList();
  }

  /// Validates a URL template. Auto-detects XYZ vs WMS and applies the
  /// appropriate placeholder/param checks. Returns null when valid; a short
  /// error key the UI can localize otherwise.
  ///
  /// XYZ: must contain `{z}`, `{x}`, `{y}` plus an http(s) scheme.
  /// WMS: must contain `{bbox}` plus `service=wms` and `layers=...` query
  ///   parameters (case-insensitive) and an http(s) scheme.
  static String? validateUrlTemplate(String template) {
    if (template.trim().isEmpty) return 'empty';
    final uri = Uri.tryParse(template);
    if (uri == null || (uri.scheme != 'http' && uri.scheme != 'https')) {
      return 'bad_scheme';
    }
    final kind = CustomUrlKind.detect(template);
    switch (kind) {
      case CustomUrlKind.xyz:
        if (!template.contains('{z}') ||
            !template.contains('{x}') ||
            !template.contains('{y}')) {
          return 'missing_placeholders';
        }
        return null;
      case CustomUrlKind.wms:
        if (!template.contains('{bbox}')) return 'missing_placeholders';
        final lower = template.toLowerCase();
        if (!lower.contains('service=wms')) return 'missing_wms_service';
        if (!lower.contains('layers=')) return 'missing_wms_layers';
        return null;
    }
  }
}

/// Adapter so the registry can treat a [CustomTileProvider] like any built-in.
class CustomTileProviderConfig extends TileProviderConfig {
  final CustomTileProvider source;

  CustomTileProviderConfig(this.source);

  @override
  String get id => source.id;

  @override
  String name(BuildContext context) => source.displayName;

  @override
  String description(BuildContext context) => source.urlTemplate;

  @override
  String get attributions => source.displayName;

  @override
  String get urlTemplate => source.urlTemplate;

  @override
  TileProviderCategory get category => source.category;

  @override
  double get minZoom => source.minZoom;

  @override
  double get maxZoom => source.maxZoom;

  @override
  Map<String, String>? get headers => {
        'User-Agent': kTurboUserAgent,
      };

  /// For WMS sources, parse the user-supplied URL into the
  /// [WMSTileLayerOptions] flutter_map expects. Returns null for XYZ.
  @override
  WMSTileLayerOptions? get wmsOptions {
    if (source.urlKind != CustomUrlKind.wms) return null;
    return _parseWmsOptions(source.urlTemplate);
  }
}

/// Parses a WMS GetMap URL into [WMSTileLayerOptions]. Keeps the query
/// parameters that aren't part of the named constructor args (other than the
/// ones flutter_map generates itself) under `otherParameters`.
WMSTileLayerOptions _parseWmsOptions(String template) {
  // Strip the {bbox}/{width}/{height} placeholders — flutter_map computes
  // them per tile — and parse what's left.
  final stripped = template
      .replaceAll(RegExp(r'[?&]BBOX=\{bbox\}', caseSensitive: false), '')
      .replaceAll(RegExp(r'[?&]WIDTH=\{width\}', caseSensitive: false), '')
      .replaceAll(RegExp(r'[?&]HEIGHT=\{height\}', caseSensitive: false), '');
  final uri = Uri.parse(stripped);

  // Case-insensitive view of the query params.
  final params = <String, String>{
    for (final e in uri.queryParameters.entries) e.key.toLowerCase(): e.value,
  };
  final baseUrl =
      '${uri.scheme}://${uri.authority}${uri.path}?'; // trailing ? per flutter_map convention

  final layers = (params['layers'] ?? '')
      .split(',')
      .where((s) => s.isNotEmpty)
      .toList();
  final format = params['format'] ?? 'image/png';
  final version = params['version'] ?? '1.1.1';
  final transparent =
      (params['transparent'] ?? 'true').toLowerCase() == 'true';
  final crsCode = (params['crs'] ?? params['srs'] ?? 'EPSG:3857').toUpperCase();
  final crs = crsCode == 'EPSG:4326' ? const Epsg4326() : const Epsg3857();

  // Pass-through query params: anything not consumed above and not generated
  // by WMSTileLayerOptions itself (service/request/layers/styles/format/srs/
  // crs/version/transparent).
  const consumed = {
    'service',
    'request',
    'layers',
    'styles',
    'format',
    'srs',
    'crs',
    'version',
    'transparent',
  };
  final other = <String, String>{
    for (final e in uri.queryParameters.entries)
      if (!consumed.contains(e.key.toLowerCase())) e.key: e.value,
  };

  return WMSTileLayerOptions(
    baseUrl: baseUrl,
    layers: layers,
    format: format,
    version: version,
    transparent: transparent,
    crs: crs,
    otherParameters: other,
  );
}
