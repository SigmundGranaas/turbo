import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_selection_pill.dart';
import 'package:turbo/features/search/api.dart';
import 'package:turbo/features/weather/api.dart';

/// Tabbed action sheet shown when the user long-presses the map.
///
/// Layout:
///   • Drag handle
///   • Place-info header (reverse-geocoded name + coords + close button)
///   • Two pills: Info | Weather
///   • Body: either the three action rows or an embedded weather forecast.
///
/// The reverse-geocoded name is also surfaced to the parent via
/// [onCreateMarker] as a pre-filled name; the sheet itself owns the
/// lookup so the [MainMapPage] doesn't have to thread a `Future` through
/// to `CreateLocationSheet` any more.
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

enum _PinTab { info, weather }

class _PinOptionsSheetState extends ConsumerState<PinOptionsSheet> {
  _PinTab _tab = _PinTab.info;
  late final Future<LocationSearchResult?> _reverseGeo;
  LocationSearchResult? _resolvedName;

  @override
  void initState() {
    super.initState();
    _reverseGeo =
        ref.read(reverseGeocoderProvider).findLocationByCoord(widget.point);
    _reverseGeo.then((value) {
      if (!mounted) return;
      setState(() => _resolvedName = value);
    });
  }

  @override
  Widget build(BuildContext context) {
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
          child: Column(
            children: [
              const _DragHandle(),
              _PlaceInfoHeader(
                point: widget.point,
                resolved: _resolvedName,
                resolving: _resolvedName == null,
              ),
              const SizedBox(height: AppSpacing.s),
              _PinTabBar(
                selected: _tab,
                onSelect: (t) => setState(() => _tab = t),
              ),
              const SizedBox(height: AppSpacing.s),
              Expanded(
                child: switch (_tab) {
                  _PinTab.info => _InfoBody(
                      scrollController: scrollController,
                      isNavigating: widget.isNavigating,
                      onCreateMarker: () {
                        Navigator.pop(context);
                        widget.onCreateMarker(_resolvedName?.title);
                      },
                      onMeasure: () {
                        Navigator.pop(context);
                        widget.onMeasure();
                      },
                      onNavigate: () {
                        Navigator.pop(context);
                        if (widget.isNavigating) {
                          widget.onStopNavigation();
                        } else {
                          widget.onNavigate();
                        }
                      },
                    ),
                  _PinTab.weather => EmbeddedWeatherBody(
                      position: widget.point,
                      scrollController: scrollController,
                    ),
                },
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
  final LocationSearchResult? resolved;
  final bool resolving;

  const _PlaceInfoHeader({
    required this.point,
    required this.resolved,
    required this.resolving,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    final l10n = context.l10n;
    final title =
        resolved?.title ?? (resolving ? l10n.pinSheetResolving : l10n.pinSheetSelectedLocation);
    final subtitleParts = <String>[
      if (resolved?.description != null && resolved!.description!.isNotEmpty)
        resolved!.description!,
      _formatCoord(point),
    ];
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
                  subtitleParts.join(' · '),
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

  static String _formatCoord(LatLng p) {
    return '${p.latitude.toStringAsFixed(5)}, ${p.longitude.toStringAsFixed(5)}';
  }
}

class _PinTabBar extends StatelessWidget {
  final _PinTab selected;
  final ValueChanged<_PinTab> onSelect;
  const _PinTabBar({required this.selected, required this.onSelect});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.l),
      child: Row(
        children: [
          AppSelectionPill(
            key: const Key('pin-tab-info'),
            selected: selected == _PinTab.info,
            onTap: () => onSelect(_PinTab.info),
            leadingIcon: Icons.list_alt_outlined,
            child: Text(l10n.pinSheetTabInfo),
          ),
          const SizedBox(width: AppSpacing.s),
          AppSelectionPill(
            key: const Key('pin-tab-weather'),
            selected: selected == _PinTab.weather,
            onTap: () => onSelect(_PinTab.weather),
            leadingIcon: Icons.wb_sunny_outlined,
            child: Text(l10n.pinSheetTabWeather),
          ),
        ],
      ),
    );
  }
}

class _InfoBody extends StatelessWidget {
  final ScrollController scrollController;
  final bool isNavigating;
  final VoidCallback onCreateMarker;
  final VoidCallback onMeasure;
  final VoidCallback onNavigate;

  const _InfoBody({
    required this.scrollController,
    required this.isNavigating,
    required this.onCreateMarker,
    required this.onMeasure,
    required this.onNavigate,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return ListView(
      key: const Key('pin-info-body'),
      controller: scrollController,
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.s),
      children: [
        ListTile(
          leading: const Icon(Icons.add_location_alt_outlined),
          title: Text(l10n.createNewMarkerHere),
          onTap: onCreateMarker,
        ),
        ListTile(
          leading: const Icon(Icons.straighten),
          title: Text(l10n.measureDistanceFromHere),
          onTap: onMeasure,
        ),
        ListTile(
          leading: const Icon(Icons.navigation_outlined),
          title: Text(isNavigating
              ? l10n.stopNavigation
              : l10n.navigateToHere),
          onTap: onNavigate,
        ),
      ],
    );
  }
}
