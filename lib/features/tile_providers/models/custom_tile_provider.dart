import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';

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
  final double minZoom;
  final double maxZoom;

  const CustomTileProvider({
    required this.id,
    required this.displayName,
    required this.urlTemplate,
    required this.category,
    this.minZoom = 1,
    this.maxZoom = 19,
  });

  Map<String, Object?> toJson() => {
        'id': id,
        'displayName': displayName,
        'urlTemplate': urlTemplate,
        'category': category.name,
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

  /// Validates that a URL template contains the {z}/{x}/{y} placeholders that
  /// flutter_map needs to substitute. Returns null when valid; otherwise a
  /// short error key the UI can localize.
  static String? validateUrlTemplate(String template) {
    if (template.trim().isEmpty) return 'empty';
    if (!template.contains('{z}') ||
        !template.contains('{x}') ||
        !template.contains('{y}')) {
      return 'missing_placeholders';
    }
    final uri = Uri.tryParse(template);
    if (uri == null || (uri.scheme != 'http' && uri.scheme != 'https')) {
      return 'bad_scheme';
    }
    return null;
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
}
