import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/features/search/api.dart';
import 'package:turbo/features/weather/api.dart';

/// Rich detail body for a tapped/selected coordinate: a reverse-geocoded
/// place-info header (peak / area / kommune with an "On / At / In" qualifier,
/// then coordinates + elevation) and the same weather-summary surface the
/// marker sheet uses. Rendered by the detail host above the action bar — the
/// header content that used to live inside `PinOptionsSheet`.
class CoordinateDetailBody extends ConsumerWidget {
  final LatLng point;
  const CoordinateDetailBody({super.key, required this.point});

  /// The reverse-geocoded title for [point], if resolved — used elsewhere to
  /// pre-fill a new marker's name without re-firing the lookup.
  static String? resolvedTitle(WidgetRef ref, LatLng point) {
    final async = ref.read(describeLocationProvider(GeoQuery(point)));
    return async.maybeWhen(data: (v) => v?.title, orElse: () => null);
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final async = ref.watch(describeLocationProvider(GeoQuery(point)));
    final description = async.maybeWhen(data: (v) => v, orElse: () => null);
    final resolving = async.isLoading && !async.hasValue;
    final title = description?.title.trim();
    final markerTitle =
        (title == null || title.isEmpty) ? l10n.pinSheetSelectedLocation : title;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        _PlaceInfoHeader(
          point: point,
          description: description,
          resolving: resolving,
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(
              AppSpacing.l, AppSpacing.s, AppSpacing.l, AppSpacing.s),
          child: WeatherSummaryRow(
            key: const Key('pin-sheet-weather-surface'),
            position: point,
            title: markerTitle,
          ),
        ),
      ],
    );
  }
}

class _PlaceInfoHeader extends StatelessWidget {
  final LatLng point;
  final LocationDescription? description;
  final bool resolving;

  const _PlaceInfoHeader({
    required this.point,
    required this.description,
    required this.resolving,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    final l10n = context.l10n;
    return Padding(
      padding: const EdgeInsets.fromLTRB(
          AppSpacing.l, AppSpacing.xs, AppSpacing.l, AppSpacing.xs),
      child: Row(
        children: [
          Icon(Icons.place, size: 28, color: colorScheme.primary),
          const SizedBox(width: AppSpacing.s),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  _resolveTitle(l10n),
                  key: const Key('pin-sheet-place-title'),
                  style: textTheme.titleMedium,
                  overflow: TextOverflow.ellipsis,
                  maxLines: 1,
                ),
                Text(
                  _resolveSubtitle(l10n),
                  style: textTheme.bodySmall
                      ?.copyWith(color: colorScheme.onSurfaceVariant),
                  overflow: TextOverflow.ellipsis,
                  maxLines: 1,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  String _resolveTitle(AppLocalizations l10n) {
    final d = description;
    if (d == null || d.title.trim().isEmpty) {
      return resolving ? l10n.pinSheetResolving : l10n.pinSheetSelectedLocation;
    }
    final prefix = _qualifierLabel(l10n, d.qualifier);
    return prefix == null ? d.title : '$prefix ${d.title}';
  }

  String _resolveSubtitle(AppLocalizations l10n) {
    final d = description;
    final elev = d?.elevationMeters;
    final dist = d?.distanceMeters;
    final secondary = d?.secondary;
    final kommuneText = _composeKommune(d);
    final parts = <String>[
      if (secondary != null && secondary.isNotEmpty) secondary,
      ?kommuneText,
      if (dist != null && dist > 30) _formatDistance(dist),
      _formatCoord(point),
      if (elev != null) '${elev.toStringAsFixed(0)} m',
    ];
    return parts.join(' · ');
  }

  static String? _composeKommune(LocationDescription? d) {
    if (d == null) return null;
    final k = d.kommune;
    final f = d.fylke;
    if (k == null || k.isEmpty) return null;
    if (f == null || f.isEmpty) return k;
    return '$k, $f';
  }

  static String _formatDistance(double meters) {
    if (meters < 1000) return '${meters.round()} m';
    return '${(meters / 1000).toStringAsFixed(1)} km';
  }

  static String _formatCoord(LatLng p) {
    return '${p.latitude.toStringAsFixed(5)}, ${p.longitude.toStringAsFixed(5)}';
  }

  static String? _qualifierLabel(AppLocalizations l10n, LocationQualifier? q) {
    return switch (q) {
      LocationQualifier.on => l10n.locationOn,
      LocationQualifier.atPlace => l10n.locationAt,
      LocationQualifier.inArea => l10n.locationIn,
      LocationQualifier.closeTo => null,
      LocationQualifier.near => null,
      null => null,
    };
  }
}
