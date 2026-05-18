import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/features/markers/api.dart' as marker_model;
import 'package:turbo/features/search/api.dart';
import 'package:turbo/features/weather/api.dart';

/// Action sheet shown when the user long-presses the map.
///
/// Layout:
///   • Drag handle
///   • Place-info header — reverse-geocoded title (peak / area / kommune)
///     with a "On / Close to / In" qualifier, then coordinates.
///   • Three action rows (create marker / measure / navigate)
///   • Weather summary surface — same widget the marker info sheet uses,
///     tapping opens the full forecast.
///
/// The sheet itself owns the reverse-geocode lookup so the [MainMapPage]
/// doesn't need to thread a `Future` through to [CreateLocationSheet];
/// the resolved title is also passed back through [onCreateMarker] as
/// the new-marker name prefill.
class PinOptionsSheet extends ConsumerStatefulWidget {
  final LatLng point;
  final bool isNavigating;
  final void Function(String? namePreview) onCreateMarker;
  final VoidCallback onMeasure;
  final VoidCallback onNavigate;
  final VoidCallback onStopNavigation;

  const PinOptionsSheet({
    super.key,
    required this.point,
    required this.isNavigating,
    required this.onCreateMarker,
    required this.onMeasure,
    required this.onNavigate,
    required this.onStopNavigation,
  });

  @override
  ConsumerState<PinOptionsSheet> createState() => _PinOptionsSheetState();
}

class _PinOptionsSheetState extends ConsumerState<PinOptionsSheet> {
  late final Future<LocationDescription?> _descriptionFuture;
  LocationDescription? _description;
  bool _resolving = true;

  @override
  void initState() {
    super.initState();
    _descriptionFuture = ref
        .read(reverseGeocoderProvider)
        .describeLocation(widget.point);
    _descriptionFuture.then((value) {
      if (!mounted) return;
      setState(() {
        _description = value;
        _resolving = false;
      });
    }).catchError((_) {
      if (!mounted) return;
      setState(() => _resolving = false);
    });
  }

  String _safeMarkerTitle(AppLocalizations l10n) {
    final title = _description?.title.trim() ?? '';
    return title.isEmpty ? l10n.pinSheetSelectedLocation : title;
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return DraggableScrollableSheet(
      initialChildSize: 0.55,
      minChildSize: 0.4,
      maxChildSize: 0.92,
      expand: false,
      builder: (context, scrollController) {
        return Container(
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.surface,
            borderRadius: const BorderRadius.vertical(
              top: Radius.circular(AppRadius.xl),
            ),
          ),
          // Single-page layout: header + weather scroll inside the
          // upper area; the three action buttons stay anchored to the
          // bottom of the sheet at any expansion level.
          child: Column(
            children: [
              const _DragHandle(),
              Expanded(
                child: SingleChildScrollView(
                  controller: scrollController,
                  padding: EdgeInsets.zero,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      _PlaceInfoHeader(
                        point: widget.point,
                        description: _description,
                        resolving: _resolving,
                      ),
                      Padding(
                        padding: const EdgeInsets.fromLTRB(AppSpacing.l,
                            AppSpacing.s, AppSpacing.l, AppSpacing.s),
                        child: WeatherSummaryRow(
                          key: const Key('pin-sheet-weather-surface'),
                          marker: marker_model.Marker(
                            title: _safeMarkerTitle(l10n),
                            position: widget.point,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
              SafeArea(
                top: false,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    ListTile(
                      leading: const Icon(Icons.add_location_alt_outlined),
                      title: Text(l10n.createNewMarkerHere),
                      onTap: () {
                        Navigator.pop(context);
                        widget.onCreateMarker(_description?.title);
                      },
                    ),
                    ListTile(
                      leading: const Icon(Icons.straighten),
                      title: Text(l10n.measureDistanceFromHere),
                      onTap: () {
                        Navigator.pop(context);
                        widget.onMeasure();
                      },
                    ),
                    ListTile(
                      leading: const Icon(Icons.navigation_outlined),
                      title: Text(widget.isNavigating
                          ? l10n.stopNavigation
                          : l10n.navigateToHere),
                      onTap: () {
                        Navigator.pop(context);
                        if (widget.isNavigating) {
                          widget.onStopNavigation();
                        } else {
                          widget.onNavigate();
                        }
                      },
                    ),
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

class _DragHandle extends StatelessWidget {
  const _DragHandle();
  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 10, bottom: 8),
      child: Center(
        child: Container(
          width: 40,
          height: 4,
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.onSurfaceVariant.withValues(
                  alpha: 0.4,
                ),
            borderRadius: BorderRadius.circular(AppRadius.s),
          ),
        ),
      ),
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
    final title = _resolveTitle(l10n);
    final subtitle = _resolveSubtitle(l10n);
    return Padding(
      padding: const EdgeInsets.fromLTRB(
          AppSpacing.l, AppSpacing.xs, AppSpacing.s, AppSpacing.xs),
      child: Row(
        children: [
          Icon(Icons.place, size: 28, color: colorScheme.primary),
          const SizedBox(width: AppSpacing.s),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  key: const Key('pin-sheet-place-title'),
                  style: textTheme.titleMedium,
                  overflow: TextOverflow.ellipsis,
                  maxLines: 1,
                ),
                Text(
                  subtitle,
                  style: textTheme.bodySmall
                      ?.copyWith(color: colorScheme.onSurfaceVariant),
                  overflow: TextOverflow.ellipsis,
                  maxLines: 1,
                ),
              ],
            ),
          ),
          IconButton(
            icon: const Icon(Icons.close),
            tooltip: MaterialLocalizations.of(context).closeButtonLabel,
            onPressed: () => Navigator.of(context).maybePop(),
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
    final parts = <String>[
      if (description?.secondary != null &&
          description!.secondary!.isNotEmpty)
        description!.secondary!,
      _formatCoord(point),
    ];
    return parts.join(' · ');
  }

  static String _formatCoord(LatLng p) {
    return '${p.latitude.toStringAsFixed(5)}, ${p.longitude.toStringAsFixed(5)}';
  }

  static String? _qualifierLabel(
      AppLocalizations l10n, LocationQualifier? q) {
    return switch (q) {
      LocationQualifier.on => l10n.locationOn,
      LocationQualifier.closeTo => l10n.locationCloseTo,
      LocationQualifier.atPlace => l10n.locationAt,
      LocationQualifier.inArea => l10n.locationIn,
      LocationQualifier.near => l10n.locationNear,
      null => null,
    };
  }
}
