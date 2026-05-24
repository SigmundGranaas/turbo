import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/features/auth/api.dart' as auth;

import '../data/activity_kind_registry.dart';
import '../models/activity_geometry.dart';
import '../models/activity_kind_descriptor.dart';
import 'route_drawing_screen.dart';

/// Bottom-sheet that asks "what kind of activity?" and dispatches to the
/// selected kind's create screen via its descriptor. The shell never
/// imports a specific kind feature.
///
/// Filters the candidate kinds by the seed geometry's shape: pinning a
/// Point only offers point-based kinds; promoting a recorded track
/// (LineString) only offers line-based kinds. This means a long-press
/// in the map → "Add activity here" and a saved-path "Save as activity"
/// share one widget but never confuse the user with mismatched kinds.
class ActivityCreatePicker extends ConsumerWidget {
  final ActivityGeometry seedGeometry;

  const ActivityCreatePicker({super.key, required this.seedGeometry});

  /// Convenience constructor for the long-press flow, where the seed
  /// is just a point.
  factory ActivityCreatePicker.fromPoint(LatLng point, {Key? key}) =>
      ActivityCreatePicker(
        key: key,
        seedGeometry: ActivityGeometry.fromPoint(point),
      );

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Activities are owner-scoped on the server — every endpoint
    // (create, read, update, delete, kinds catalog, conditions) requires
    // a JWT. Anonymous users can't get past the first POST, so render a
    // sign-in CTA instead of the kind list. Doing the guard here covers
    // every entry point (long-press menu, marker "save as activity",
    // saved-path promotion) at one site.
    final isAuthenticated = ref.watch(auth.authStateProvider.select(
        (s) => s.status == auth.AuthStatus.authenticated));
    if (!isAuthenticated) {
      return _SignInPrompt(seedGeometry: seedGeometry);
    }

    final registry = ref.watch(activityKindRegistryProvider);
    final kinds = registry.all
        .where((k) => k.allowedGeometries.contains(seedGeometry.kind))
        .toList();
    // When the user long-pressed (point seed) we also surface line-based
    // kinds under a "draw a route" affordance — otherwise hiking,
    // xc_ski, etc. are unreachable from the map and the user has to
    // know to start from the recording feature.
    final routeKinds = seedGeometry.kind == ActivityGeometryKind.point
        ? registry.all
            .where((k) =>
                k.allowedGeometries.contains(ActivityGeometryKind.lineString))
            .toList()
        : const <ActivityKindDescriptor>[];
    final seedPoint = seedGeometry.firstPoint;
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 16, 16, 24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'New activity',
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            const SizedBox(height: 8),
            Text(
              _subtitleFor(seedGeometry.kind),
              style: Theme.of(context).textTheme.bodyMedium,
            ),
            const SizedBox(height: 16),
            if (kinds.isEmpty && routeKinds.isEmpty)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 24),
                child: Text(_emptyMessageFor(seedGeometry.kind)),
              )
            else ...[
              ...kinds.map((k) => _KindTile(
                    descriptor: k,
                    seedGeometry: seedGeometry,
                  )),
              if (routeKinds.isNotEmpty && seedPoint != null) ...[
                const SizedBox(height: 8),
                const Divider(),
                const SizedBox(height: 8),
                Text(
                  'Or draw a route from here',
                  style: Theme.of(context).textTheme.labelLarge,
                ),
                const SizedBox(height: 4),
                ...routeKinds.map((k) => _RouteKindTile(
                      descriptor: k,
                      seedPoint: seedPoint,
                    )),
              ],
            ],
          ],
        ),
      ),
    );
  }

  static String _subtitleFor(ActivityGeometryKind kind) => switch (kind) {
        ActivityGeometryKind.point =>
          'Pick a kind — each kind has its own form, data, and conditions.',
        ActivityGeometryKind.lineString =>
          'Pick a kind to record this route as.',
        ActivityGeometryKind.polygon =>
          'Pick a kind to record this area as.',
      };

  static String _emptyMessageFor(ActivityGeometryKind kind) => switch (kind) {
        ActivityGeometryKind.point =>
          'No point-based activity kinds are registered in this build.',
        ActivityGeometryKind.lineString =>
          'No route-based activity kinds are registered in this build.',
        ActivityGeometryKind.polygon =>
          'No area-based activity kinds are registered in this build.',
      };
}

class _KindTile extends StatelessWidget {
  final ActivityKindDescriptor descriptor;
  final ActivityGeometry seedGeometry;
  const _KindTile({required this.descriptor, required this.seedGeometry});

  @override
  Widget build(BuildContext context) {
    return ListTile(
      contentPadding: EdgeInsets.zero,
      leading: CircleAvatar(
        backgroundColor: descriptor.tintColor.withValues(alpha: 0.15),
        foregroundColor: descriptor.tintColor,
        child: Icon(descriptor.icon),
      ),
      title: Text(descriptor.displayName),
      trailing: const Icon(Icons.chevron_right),
      onTap: () async {
        // Push the create screen on top of the picker, then pop the
        // picker with the create screen's result. Lets parent sheets
        // (path detail, marker detail) auto-dismiss on save so the
        // user lands back on the map and sees their new pin without
        // an extra "close this sheet" tap.
        final navigator = Navigator.of(context);
        final saved = await navigator.push<bool>(MaterialPageRoute(
          builder: (ctx) => descriptor.buildCreateScreen(ctx, seedGeometry),
        ));
        if (context.mounted) navigator.pop(saved);
      },
    );
  }
}

/// Variant of [_KindTile] for the long-press flow's route-based kinds:
/// tapping launches [RouteDrawingScreen] first, then opens the kind's
/// create screen seeded with the drawn LineString. Lets users start a
/// hike / xc-ski / packraft from any map long-press without already
/// having a recorded track.
class _RouteKindTile extends StatelessWidget {
  final ActivityKindDescriptor descriptor;
  final LatLng seedPoint;
  const _RouteKindTile({required this.descriptor, required this.seedPoint});

  @override
  Widget build(BuildContext context) {
    return ListTile(
      contentPadding: EdgeInsets.zero,
      leading: CircleAvatar(
        backgroundColor: descriptor.tintColor.withValues(alpha: 0.15),
        foregroundColor: descriptor.tintColor,
        child: Icon(descriptor.icon),
      ),
      title: Text(descriptor.displayName),
      subtitle: const Text('Draw the route on the map'),
      trailing: const Icon(Icons.timeline_outlined),
      onTap: () async {
        final navigator = Navigator.of(context);
        final route = await navigator.push<List<LatLng>>(
          MaterialPageRoute(
            builder: (ctx) => RouteDrawingScreen(
              seedCenter: seedPoint,
              color: descriptor.tintColor,
            ),
          ),
        );
        if (route == null || route.length < 2) return;
        if (!context.mounted) return;
        final saved = await navigator.push<bool>(MaterialPageRoute(
          builder: (ctx) => descriptor.buildCreateScreen(
              ctx, ActivityGeometry.fromRoute(route)),
        ));
        if (context.mounted) navigator.pop(saved);
      },
    );
  }
}

class _SignInPrompt extends StatelessWidget {
  final ActivityGeometry seedGeometry;
  const _SignInPrompt({required this.seedGeometry});

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 16, 16, 24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Sign in to add activities',
                style: Theme.of(context).textTheme.headlineSmall),
            const SizedBox(height: 8),
            Text(
              _bodyFor(seedGeometry.kind),
              style: Theme.of(context).textTheme.bodyMedium,
            ),
            const SizedBox(height: 16),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                TextButton(
                  onPressed: () => Navigator.of(context).pop(),
                  child: const Text('Not now'),
                ),
                const SizedBox(width: 8),
                FilledButton.icon(
                  icon: const Icon(Icons.login, size: 18),
                  label: const Text('Sign in'),
                  onPressed: () {
                    Navigator.of(context).pop();
                    auth.LoginScreen.show(context);
                  },
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  static String _bodyFor(ActivityGeometryKind kind) => switch (kind) {
        ActivityGeometryKind.point =>
          'Activities are saved to your account so you can revisit them with current conditions. Sign in to add one here.',
        ActivityGeometryKind.lineString =>
          'Activities are saved to your account so you can revisit them with current conditions. Sign in to save this route as an activity.',
        ActivityGeometryKind.polygon =>
          'Activities are saved to your account so you can revisit them with current conditions. Sign in to save this area as an activity.',
      };
}
