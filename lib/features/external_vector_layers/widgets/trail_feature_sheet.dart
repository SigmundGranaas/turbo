import 'package:flutter/material.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import '../models/vector_feature.dart';
import 'trail_property_decoder.dart';

/// Bottom sheet body for a tapped Turrutebasen feature
/// (`Fotrute` / `Skiløype` / `Sykkelrute` / `AnnenRute`).
///
/// Visual hierarchy (top → bottom):
///   1. Subtype label — small, muted.
///   2. Route name — `titleLarge`, the hero.
///   3. Route number — bodyMedium, muted (rendered only if present).
///   4. Status chips row — marking, difficulty, season, preparation
///      (skiløype only). Hidden when there's nothing to show.
///   5. Detail rows — maintained-by, surface, follows, notes. Only
///      rendered for keys that carry a non-empty SOSI value.
///   6. Source footer — origin + updated date, smallest weight.
///
/// Anything not in this list is intentionally hidden: GUID identifiers,
/// measurement-method codes, internal namespaces, etc. live in the raw
/// GeoJSON properties but never make it into the UI.
class TrailFeatureSheet extends StatelessWidget {
  final VectorFeature feature;

  /// Localised subtype label for the column above the name ("Hiking trail",
  /// "Ski track", etc.). Supplied by the source so the sheet stays generic
  /// across the four trail subtypes.
  final String subtypeLabel;

  /// Source accent colour — used as the left-edge tick and for chip
  /// outlines so users can see at a glance which layer the feature
  /// belongs to.
  final Color accent;

  const TrailFeatureSheet({
    super.key,
    required this.feature,
    required this.subtypeLabel,
    required this.accent,
  });

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final tt = Theme.of(context).textTheme;
    final l10n = context.l10n;

    final decoded = TrailProperties.from(feature, context);

    return DraggableScrollableSheet(
      initialChildSize: 0.45,
      minChildSize: 0.25,
      maxChildSize: 0.9,
      expand: false,
      builder: (context, controller) {
        return Container(
          decoration: BoxDecoration(
            color: scheme.surface,
            borderRadius: const BorderRadius.vertical(
                top: Radius.circular(AppRadius.xl)),
          ),
          child: ListView(
            controller: controller,
            padding: const EdgeInsets.fromLTRB(
                AppSpacing.xl, AppSpacing.m, AppSpacing.xl, AppSpacing.xl),
            children: [
              _DragHandle(),
              const SizedBox(height: AppSpacing.m),
              _Header(
                subtypeLabel: subtypeLabel,
                title: decoded.title ?? l10n.trailNameUnknown,
                routeNumber: decoded.routeNumber,
                accent: accent,
              ),
              if (decoded.hasChips) ...[
                const SizedBox(height: AppSpacing.l),
                _ChipsRow(decoded: decoded, accent: accent),
              ],
              if (decoded.hasDetails) ...[
                const SizedBox(height: AppSpacing.l),
                Divider(color: scheme.outlineVariant, height: 1),
                const SizedBox(height: AppSpacing.l),
                ..._detailRows(context, decoded),
              ],
              if (decoded.source != null || decoded.updated != null) ...[
                const SizedBox(height: AppSpacing.l),
                _SourceFooter(
                  source: decoded.source,
                  updated: decoded.updated,
                ),
              ],
              if (!decoded.hasChips &&
                  !decoded.hasDetails &&
                  decoded.routeNumber == null) ...[
                const SizedBox(height: AppSpacing.l),
                Text(
                  l10n.trailDetailEmpty,
                  style: tt.bodyMedium?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              ],
            ],
          ),
        );
      },
    );
  }

  List<Widget> _detailRows(BuildContext context, TrailProperties d) {
    final l10n = context.l10n;
    final rows = <(String, String)>[
      if (d.maintainer != null) (l10n.trailMaintainerLabel, d.maintainer!),
      if (d.follows != null) (l10n.trailFollowsLabel, d.follows!),
      if (d.surface != null) (l10n.trailSurfaceLabel, d.surface!),
      if (d.width != null) (l10n.trailLengthLabel, d.width!),
      if (d.notes != null) (l10n.trailNotesLabel, d.notes!),
    ];
    return [
      for (final r in rows) ...[
        _DetailRow(label: r.$1, value: r.$2),
        const SizedBox(height: AppSpacing.s),
      ],
    ];
  }
}

class _DragHandle extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Center(
      child: Container(
        width: 40,
        height: 4,
        decoration: BoxDecoration(
          color: Theme.of(context)
              .colorScheme
              .onSurfaceVariant
              .withValues(alpha: 0.4),
          borderRadius: BorderRadius.circular(AppRadius.s),
        ),
      ),
    );
  }
}

class _Header extends StatelessWidget {
  final String subtypeLabel;
  final String title;
  final String? routeNumber;
  final Color accent;

  const _Header({
    required this.subtypeLabel,
    required this.title,
    required this.routeNumber,
    required this.accent,
  });

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final tt = Theme.of(context).textTheme;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          width: 4,
          height: 44,
          margin: const EdgeInsets.only(right: AppSpacing.m, top: 2),
          decoration: BoxDecoration(
            color: accent,
            borderRadius: BorderRadius.circular(2),
          ),
        ),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                subtypeLabel.toUpperCase(),
                style: tt.labelSmall?.copyWith(
                  color: scheme.onSurfaceVariant,
                  letterSpacing: 1.1,
                ),
              ),
              const SizedBox(height: AppSpacing.xs),
              Text(
                title,
                style: tt.titleLarge?.copyWith(fontWeight: FontWeight.w600),
              ),
              if (routeNumber != null) ...[
                const SizedBox(height: 2),
                Text(
                  routeNumber!,
                  style: tt.bodyMedium?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              ],
            ],
          ),
        ),
      ],
    );
  }
}

class _ChipsRow extends StatelessWidget {
  final TrailProperties decoded;
  final Color accent;

  const _ChipsRow({required this.decoded, required this.accent});

  @override
  Widget build(BuildContext context) {
    final chips = <_ChipSpec>[
      if (decoded.marking != null) _ChipSpec(decoded.marking!, Icons.signpost),
      if (decoded.difficulty != null)
        _ChipSpec(decoded.difficulty!.label, Icons.terrain,
            color: decoded.difficulty!.color),
      if (decoded.season != null) _ChipSpec(decoded.season!, Icons.eco),
      if (decoded.preparation != null)
        _ChipSpec(decoded.preparation!, Icons.ac_unit),
    ];
    return Wrap(
      spacing: AppSpacing.s,
      runSpacing: AppSpacing.s,
      children: [
        for (final c in chips) _Chip(spec: c, fallbackAccent: accent),
      ],
    );
  }
}

class _ChipSpec {
  final String label;
  final IconData icon;
  final Color? color;
  const _ChipSpec(this.label, this.icon, {this.color});
}

class _Chip extends StatelessWidget {
  final _ChipSpec spec;
  final Color fallbackAccent;

  const _Chip({required this.spec, required this.fallbackAccent});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final tt = Theme.of(context).textTheme;
    final tone = spec.color ?? scheme.onSurfaceVariant;
    return Container(
      padding: const EdgeInsets.symmetric(
          horizontal: AppSpacing.m, vertical: AppSpacing.xs + 2),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(AppRadius.pill),
        border: Border.all(color: tone.withValues(alpha: 0.35)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(spec.icon, size: 14, color: tone),
          const SizedBox(width: 6),
          Text(
            spec.label,
            style: tt.labelMedium?.copyWith(color: scheme.onSurface),
          ),
        ],
      ),
    );
  }
}

class _DetailRow extends StatelessWidget {
  final String label;
  final String value;

  const _DetailRow({required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final tt = Theme.of(context).textTheme;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          label,
          style: tt.labelMedium?.copyWith(color: scheme.onSurfaceVariant),
        ),
        const SizedBox(height: 2),
        Text(value, style: tt.bodyMedium),
      ],
    );
  }
}

class _SourceFooter extends StatelessWidget {
  final String? source;
  final String? updated;

  const _SourceFooter({required this.source, required this.updated});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final tt = Theme.of(context).textTheme;
    final l10n = context.l10n;
    final parts = <String>[
      if (source != null) '${l10n.trailSourceLabel}: $source',
      if (updated != null) '${l10n.trailUpdatedLabel}: $updated',
    ];
    return Text(
      parts.join(' · '),
      style: tt.bodySmall?.copyWith(color: scheme.onSurfaceVariant),
    );
  }
}
