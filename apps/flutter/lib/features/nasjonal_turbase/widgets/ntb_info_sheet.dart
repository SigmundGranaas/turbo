import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/sheet_drag_handle.dart';
import '../models/ntb_poi.dart';
import '../models/ntb_route.dart';
import '../providers/ntb_providers.dart';

/// Info sheet for a Nasjonal Turbase POI. Shows the name, an image, a summary,
/// (for trips) route metadata once the route has loaded, an attribution line,
/// and an "Open in UT.no" button. Read-only — this is browsing data sourced
/// from ut.no / DNT, not user content.
class NtbInfoSheet extends ConsumerWidget {
  final NtbPoi poi;

  const NtbInfoSheet({super.key, required this.poi});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scheme = Theme.of(context).colorScheme;
    final tt = Theme.of(context).textTheme;
    final l10n = context.l10n;

    // For a trip, surface the richer metadata once its route has loaded.
    final selection = ref.watch(ntbSelectedRouteProvider);
    final NtbRoute? route =
        (selection?.poi.id == poi.id) ? selection?.route : null;

    final description = route?.description ?? poi.summary;
    final imageUrl = route?.imageUrl ?? poi.imageUrl;
    final utUrl = route?.utUrl ?? poi.utUrl;

    return DraggableScrollableSheet(
      initialChildSize: 0.4,
      minChildSize: 0.2,
      maxChildSize: 0.85,
      expand: false,
      builder: (context, controller) {
        return Container(
          decoration: BoxDecoration(
            color: scheme.surface,
            borderRadius: const BorderRadius.vertical(top: Radius.circular(24)),
          ),
          child: ListView(
            controller: controller,
            padding: const EdgeInsets.fromLTRB(20, 16, 20, 24),
            children: [
              const SheetDragHandle(),
              const SizedBox(height: 16),
              Text(
                _typeLabel(l10n, poi.type),
                style: tt.labelMedium?.copyWith(color: scheme.onSurfaceVariant),
              ),
              const SizedBox(height: 4),
              Text(poi.title, style: tt.titleLarge),
              const SizedBox(height: 16),
              if (imageUrl != null) ...[
                ClipRRect(
                  borderRadius: BorderRadius.circular(12),
                  child: Image.network(
                    imageUrl,
                    height: 160,
                    width: double.infinity,
                    fit: BoxFit.cover,
                    errorBuilder: (_, _, _) => const SizedBox.shrink(),
                  ),
                ),
                const SizedBox(height: 16),
              ],
              if (route != null) _routeMeta(context, route, l10n),
              if (description != null)
                Text(description, style: tt.bodyMedium)
              else if (poi.hasRoute && route == null)
                Row(
                  children: [
                    const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                    const SizedBox(width: 12),
                    Text(l10n.ntbLoadingRoute,
                        style: tt.bodyMedium
                            ?.copyWith(color: scheme.onSurfaceVariant)),
                  ],
                ),
              const SizedBox(height: 20),
              if (utUrl != null)
                FilledButton.icon(
                  onPressed: () => launchUrl(
                    Uri.parse(utUrl),
                    mode: LaunchMode.externalApplication,
                  ),
                  icon: const Icon(Icons.open_in_new),
                  label: Text(l10n.ntbOpenInUt),
                ),
              const SizedBox(height: 16),
              Text(
                l10n.ntbAttribution,
                style: tt.bodySmall?.copyWith(color: scheme.onSurfaceVariant),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _routeMeta(BuildContext context, NtbRoute route, AppLocalizations l10n) {
    final tt = Theme.of(context).textTheme;
    final scheme = Theme.of(context).colorScheme;
    final rows = <Widget>[];

    void add(String label, String value) {
      rows.add(Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SizedBox(
              width: 110,
              child: Text(label,
                  style:
                      tt.bodySmall?.copyWith(color: scheme.onSurfaceVariant)),
            ),
            Expanded(child: Text(value, style: tt.bodyMedium)),
          ],
        ),
      ));
    }

    final dist = route.distanceMeters;
    if (dist != null && dist > 0) {
      add(l10n.ntbDistanceLabel, _formatDistance(dist));
    }
    if (route.grade != null) add(l10n.ntbGradeLabel, route.grade!);

    if (rows.isEmpty) return const SizedBox.shrink();
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: rows),
    );
  }

  static String _formatDistance(double meters) {
    if (meters >= 1000) {
      final km = meters / 1000;
      return '${km.toStringAsFixed(km >= 10 ? 0 : 1)} km';
    }
    return '${meters.round()} m';
  }

  static String _typeLabel(AppLocalizations l10n, NtbPoiType type) =>
      switch (type) {
        NtbPoiType.cabin => l10n.ntbTypeCabin,
        NtbPoiType.trip => l10n.ntbTypeTrip,
        NtbPoiType.place => l10n.ntbTypePlace,
      };
}
