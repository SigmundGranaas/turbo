import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/sheet_drag_handle.dart';
import '../models/vector_feature.dart';
import '../models/vector_layer_source.dart';

/// Opens the bottom sheet for [feature]. If the [source] supplied a
/// [VectorLayerSource.sheetBuilder] the source-specific layout is used
/// (e.g. trails get the rich [TrailFeatureSheet]); otherwise the
/// generic key/value [VectorFeatureSheet] takes over.
Future<void> showVectorFeatureSheet(
  BuildContext context, {
  required VectorLayerSource source,
  required VectorFeature feature,
  List<String>? shownKeys,
  Map<String, String> Function(BuildContext)? labelOverrides,
}) {
  return showExclusiveSheet<void>(
    context,
    backgroundColor: Colors.transparent,
    builder: (sheetContext) {
      final custom = source.sheetBuilder;
      if (custom != null) return custom(sheetContext, feature);
      return VectorFeatureSheet(
        source: source,
        feature: feature,
        shownKeys: shownKeys,
        labelOverrides: labelOverrides,
      );
    },
  );
}

class VectorFeatureSheet extends StatelessWidget {
  final VectorLayerSource source;
  final VectorFeature feature;
  final List<String>? shownKeys;
  final Map<String, String> Function(BuildContext)? labelOverrides;

  const VectorFeatureSheet({
    super.key,
    required this.source,
    required this.feature,
    this.shownKeys,
    this.labelOverrides,
  });

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final tt = Theme.of(context).textTheme;
    final l10n = context.l10n;

    final overrides = labelOverrides?.call(context) ?? const {};
    final entries = _selectEntries(overrides);
    final title = _bestTitle();

    return DraggableScrollableSheet(
      initialChildSize: 0.4,
      minChildSize: 0.2,
      maxChildSize: 0.85,
      expand: false,
      builder: (context, controller) {
        return Container(
          decoration: BoxDecoration(
            color: scheme.surface,
            borderRadius:
                const BorderRadius.vertical(top: Radius.circular(24)),
          ),
          child: ListView(
            controller: controller,
            padding: const EdgeInsets.fromLTRB(20, 16, 20, 24),
            children: [
              const SheetDragHandle(),
              const SizedBox(height: 16),
              Text(source.name(context), style: tt.labelMedium?.copyWith(
                color: scheme.onSurfaceVariant,
              )),
              const SizedBox(height: 4),
              Text(title, style: tt.titleLarge),
              const SizedBox(height: 16),
              if (entries.isEmpty)
                Text(
                  l10n.trailDetailEmpty,
                  style: tt.bodyMedium?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              for (final entry in entries) ...[
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 4),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      SizedBox(
                        width: 130,
                        child: Text(
                          entry.key,
                          style: tt.bodySmall?.copyWith(
                            color: scheme.onSurfaceVariant,
                          ),
                        ),
                      ),
                      Expanded(
                        child: Text(entry.value, style: tt.bodyMedium),
                      ),
                    ],
                  ),
                ),
              ],
            ],
          ),
        );
      },
    );
  }

  String _bestTitle() {
    for (final key in const ['navn', 'name', 'title', 'event']) {
      final v = feature.properties[key];
      if (v != null && v.toString().trim().isNotEmpty) return v.toString();
    }
    return feature.id;
  }

  List<MapEntry<String, String>> _selectEntries(Map<String, String> overrides) {
    final raw = feature.properties.entries
        .where((e) => e.value != null)
        .where((e) => e.value.toString().trim().isNotEmpty)
        .toList();
    final keys = shownKeys ?? raw.map((e) => e.key).toList();
    return [
      for (final key in keys)
        if (feature.properties[key] != null)
          MapEntry(
            overrides[key] ?? key,
            feature.properties[key].toString(),
          ),
    ];
  }
}
