import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/features/activities/api.dart';

import '../data/today_recommendations_api.dart';
import '../models/recommendation_item.dart';
import '../models/today_query.dart';
import 'today_card.dart';

/// "What's good near me right now" — ranked across kinds. The textual
/// counterpart to the map's score-shaded pins.
///
/// Takes [location] from the caller (typically the device's last known
/// position; the existing map screen already plumbs this). Falls back to
/// a clear empty state when no location is available rather than asking
/// for permissions mid-flow.
class TodayScreen extends ConsumerStatefulWidget {
  /// Caller-supplied current location. Pass null to render the
  /// "location required" empty state — Today is a "near me" view and
  /// has no meaning without coordinates.
  final LatLng? location;
  final Color appBarTint;

  const TodayScreen({
    super.key,
    required this.location,
    this.appBarTint = const Color(0xFF1F6BB7),
  });

  @override
  ConsumerState<TodayScreen> createState() => _TodayScreenState();
}

class _TodayScreenState extends ConsumerState<TodayScreen> {
  Set<String>? _kindFilter; // null = all
  double _radiusKm = 25.0;

  @override
  Widget build(BuildContext context) {
    final location = widget.location;
    return Scaffold(
      appBar: AppBar(
        title: const Text('Today'),
        backgroundColor: widget.appBarTint.withValues(alpha: 0.08),
        actions: [
          IconButton(
            icon: const Icon(Icons.tune),
            tooltip: 'Filters',
            onPressed: () => _openFilters(context),
          ),
        ],
      ),
      body: SafeArea(
        child: location == null
            ? const _NoLocation()
            : _RecommendationsList(
                query: TodayQuery(
                  location: location,
                  at: _today(),
                  kinds: _kindFilter,
                  radiusKm: _radiusKm,
                ),
                onTap: _openDetail,
              ),
      ),
    );
  }

  /// Truncate to the start of the local hour. Keeps the family key
  /// stable enough that pulling-to-refresh doesn't churn the cache.
  static DateTime _today() {
    final now = DateTime.now();
    return DateTime(now.year, now.month, now.day, now.hour);
  }

  Future<void> _openFilters(BuildContext context) async {
    await showExclusiveSheet<void>(
      context,
      builder: (ctx) => _FilterSheet(
        kinds: _kindFilter,
        radiusKm: _radiusKm,
        onChanged: (kinds, radius) {
          setState(() {
            _kindFilter = kinds;
            _radiusKm = radius;
          });
        },
      ),
    );
  }

  Future<void> _openDetail(BuildContext context, RecommendationItem item) async {
    final registry = ref.read(activityKindRegistryProvider);
    final descriptor = registry.get(item.kind);
    if (descriptor == null) return;
    final id = item.activityId;
    if (id == null) return; // Discovery-only items will plug in later.
    Navigator.of(context).push(MaterialPageRoute<void>(
      builder: (ctx) => descriptor.buildDetailContent != null
          ? _PushDetail(descriptor: descriptor, activityId: id)
          : descriptor.buildDetailScreen(ctx, id),
    ));
  }
}

/// Thin wrapper because we can't directly construct ActivityDetailScreen
/// from this layer without importing it from the activities feature —
/// instead the registry's descriptor knows how. We re-use the same
/// path the map layer uses.
class _PushDetail extends StatelessWidget {
  final ActivityKindDescriptor descriptor;
  final String activityId;
  const _PushDetail({required this.descriptor, required this.activityId});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(descriptor.displayName),
        backgroundColor: descriptor.tintColor.withValues(alpha: 0.08),
      ),
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.only(bottom: 24),
          child: descriptor.buildDetailContent!(context, activityId),
        ),
      ),
    );
  }
}

class _RecommendationsList extends ConsumerWidget {
  final TodayQuery query;
  final void Function(BuildContext, RecommendationItem) onTap;

  const _RecommendationsList({required this.query, required this.onTap});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(todayRecommendationsProvider(query));
    final registry = ref.watch(activityKindRegistryProvider);
    return async.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (e, _) => _ErrorState(message: e.toString(), onRetry: () {
        ref.invalidate(todayRecommendationsProvider(query));
      }),
      data: (response) {
        if (response.items.isEmpty) return const _EmptyState();
        return ListView.builder(
          padding: const EdgeInsets.symmetric(vertical: 8),
          itemCount: response.items.length,
          itemBuilder: (ctx, i) {
            final item = response.items[i];
            final tint = registry.get(item.kind)?.tintColor ?? Colors.indigo;
            return TodayCard(
              item: item,
              tintColor: tint,
              onTap: () => onTap(ctx, item),
            );
          },
        );
      },
    );
  }
}

class _NoLocation extends StatelessWidget {
  const _NoLocation();
  @override
  Widget build(BuildContext context) => const Center(
        child: Padding(
          padding: EdgeInsets.all(32),
          child: Text(
            'Location required to surface today\'s recommendations.\n\nOpen the map to set your position.',
            textAlign: TextAlign.center,
          ),
        ),
      );
}

class _EmptyState extends StatelessWidget {
  const _EmptyState();
  @override
  Widget build(BuildContext context) => Center(
        child: Padding(
          padding: const EdgeInsets.all(32),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.explore_off_outlined, size: 48, color: Theme.of(context).colorScheme.outline),
              const SizedBox(height: 12),
              const Text(
                'Nothing scored well in this radius.\nTry widening the filter or creating activities.',
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      );
}

class _ErrorState extends StatelessWidget {
  final String message;
  final VoidCallback onRetry;
  const _ErrorState({required this.message, required this.onRetry});

  @override
  Widget build(BuildContext context) => Center(
        child: Padding(
          padding: const EdgeInsets.all(32),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.cloud_off_outlined, size: 48, color: Theme.of(context).colorScheme.outline),
              const SizedBox(height: 12),
              Text(
                'Recommendations need a connection.\n\n$message',
                textAlign: TextAlign.center,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
              const SizedBox(height: 12),
              FilledButton.icon(
                onPressed: onRetry,
                icon: const Icon(Icons.refresh),
                label: const Text('Retry'),
              ),
            ],
          ),
        ),
      );
}

class _FilterSheet extends StatefulWidget {
  final Set<String>? kinds;
  final double radiusKm;
  final void Function(Set<String>? kinds, double radius) onChanged;

  const _FilterSheet({
    required this.kinds,
    required this.radiusKm,
    required this.onChanged,
  });

  @override
  State<_FilterSheet> createState() => _FilterSheetState();
}

class _FilterSheetState extends State<_FilterSheet> {
  late Set<String> _selected;
  late double _radius;

  static const _kindOptions = <(String, String)>[
    ('xc_ski', 'XC skiing'),
    ('backcountry_ski', 'Backcountry'),
    ('freediving', 'Freediving'),
    ('fishing', 'Fishing'),
    ('hiking', 'Hiking'),
    ('packrafting', 'Packrafting'),
  ];

  @override
  void initState() {
    super.initState();
    _selected = widget.kinds == null
        ? _kindOptions.map((k) => k.$1).toSet()
        : widget.kinds!.toSet();
    _radius = widget.radiusKm;
  }

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(20, 16, 20, 24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Filters', style: Theme.of(context).textTheme.titleLarge),
            const SizedBox(height: 12),
            const Text('Kinds'),
            const SizedBox(height: 6),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                for (final (key, label) in _kindOptions)
                  FilterChip(
                    label: Text(label),
                    selected: _selected.contains(key),
                    onSelected: (s) => setState(() {
                      if (s) {
                        _selected.add(key);
                      } else {
                        _selected.remove(key);
                      }
                    }),
                  ),
              ],
            ),
            const SizedBox(height: 16),
            Text('Radius: ${_radius.round()} km'),
            Slider(
              min: 5,
              max: 100,
              divisions: 19,
              value: _radius,
              label: '${_radius.round()} km',
              onChanged: (v) => setState(() => _radius = v),
            ),
            const SizedBox(height: 12),
            FilledButton(
              onPressed: () {
                final kinds = _selected.length == _kindOptions.length ? null : _selected;
                widget.onChanged(kinds, _radius);
                Navigator.of(context).pop();
              },
              child: const Text('Apply'),
            ),
          ],
        ),
      ),
    );
  }
}
