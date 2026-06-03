import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart' hide DistanceCalculator;

import 'package:turbo/core/location/compass_mode_state.dart';
import 'package:turbo/core/location/compass_state.dart';
import 'package:turbo/core/location/follow_mode_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_pill.dart';
import 'package:turbo/features/journey/api.dart';
import 'package:turbo/features/navigation/api.dart';
import 'package:turbo/features/path_recording/api.dart';
import 'package:turbo/features/settings/api.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

/// A single adaptive "what am I doing right now" chip.
///
/// Previously this stacked up to four separate chips (journey, point-nav,
/// follow, compass). Those are facets of one live state, so they're now
/// composed into ONE chip showing the dominant mode, with the compass heading
/// folded in as an inline badge rather than a second stacked row. Priority:
/// following a path → navigating to a point → follow (snap) → compass-only.
/// See `docs/architecture/2026-06-composition-overhaul-plan.md` (Phase 3,
/// state-combining pass).
class ModeIndicator extends ConsumerWidget {
  const ModeIndicator({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final follow = ref.watch(followModeProvider);
    final compassOn = ref.watch(compassModeProvider);
    final heading = ref.watch(compassStateProvider).value;
    final journey = ref.watch(activeJourneyProvider);
    final nav = ref.watch(navigationStateProvider);
    final scheme = Theme.of(context).colorScheme;
    final l10n = context.l10n;

    // This chip is orientation-only now: following-a-path and recording are
    // owned by the single ActiveOutingPanel. Hide entirely while an outing is
    // active so the two never stack.
    final outingActive = journey.kind == JourneyKind.followingPath ||
        ref.watch(recordingNotifierProvider).isActive;

    // Compass folds into whatever the dominant chip is, as an inline badge.
    final Widget? compassBadge = (compassOn && heading != null)
        ? _CompassBadge(heading: heading, scheme: scheme)
        : null;

    // 1) Point-to-point navigation (computes live bearing → its own chip).
    if (nav.isActive && nav.target != null) {
      return _NavChip(
        target: nav.target!,
        compassHeading: heading,
        scheme: scheme,
        trailing: compassBadge,
        onDismiss: () =>
            ref.read(navigationStateProvider.notifier).stopNavigation(),
      );
    }

    // 2) Follow (snap-to-me) without a destination — hidden during an outing.
    if (follow.isOnOrPaused && !outingActive) {
      final paused = follow == FollowMode.paused;
      return _StatusChip(
        icon: paused ? Icons.location_searching : Icons.my_location,
        scheme: scheme,
        primary: paused ? '${l10n.following} · paused' : l10n.following,
        trailing: compassBadge,
        onTap: paused
            ? () => ref.read(followModeProvider.notifier).resume()
            : null,
        onDismiss: () => ref.read(followModeProvider.notifier).disable(),
      );
    }

    // 4) Compass orientation only.
    if (compassOn) {
      return _StatusChip(
        icon: Icons.explore,
        scheme: scheme,
        primary: heading != null
            ? '${_cardinal(heading)} ${heading.round()}°'
            : l10n.compassOrientation,
        onDismiss: () => ref.read(compassModeProvider.notifier).disable(),
      );
    }

    return const SizedBox.shrink();
  }
}

/// Generic single-row chip: leading icon, primary (+ optional secondary)
/// label, an optional trailing badge, and a dismiss button (48dp target).
class _StatusChip extends StatelessWidget {
  final IconData icon;
  final ColorScheme scheme;
  final String primary;
  final Widget? trailing;
  final VoidCallback? onTap;
  final VoidCallback onDismiss;

  const _StatusChip({
    required this.icon,
    required this.scheme,
    required this.primary,
    required this.onDismiss,
    this.trailing,
    this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final body = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, color: scheme.onSurfaceVariant),
        const SizedBox(width: AppSpacing.m),
        Flexible(
          child: Text(
            primary,
            overflow: TextOverflow.ellipsis,
            style: textTheme.titleSmall
                ?.copyWith(fontWeight: FontWeight.w500, color: scheme.onSurface),
          ),
        ),
        if (trailing != null) ...[
          const SizedBox(width: AppSpacing.s),
          trailing!,
        ],
        IconButton(
          visualDensity: VisualDensity.compact,
          onPressed: onDismiss,
          icon: Icon(Icons.close, size: 20, color: scheme.onSurfaceVariant),
        ),
      ],
    );
    final pill = AppPill(child: body);
    if (onTap == null) return pill;
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(AppRadius.xl),
      child: pill,
    );
  }
}

/// Point-navigation chip — needs live position to compute distance + turn.
class _NavChip extends ConsumerWidget {
  final LatLng target;
  final double? compassHeading;
  final ColorScheme scheme;
  final Widget? trailing;
  final VoidCallback onDismiss;

  const _NavChip({
    required this.target,
    required this.compassHeading,
    required this.scheme,
    required this.onDismiss,
    this.trailing,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final userPosition = ref.watch(locationStateProvider).value;
    final textTheme = Theme.of(context).textTheme;
    final l10n = context.l10n;
    if (userPosition == null) return const SizedBox.shrink();

    final distanceM = const Distance().distance(userPosition, target);
    final unit = ref.watch(settingsProvider
        .select((s) => s.value?.distanceUnit ?? DistanceUnit.metric));
    final distanceText = formatDistance(distanceM, unit);
    final bearingToTarget = const Distance().bearing(userPosition, target);

    String directionText;
    double arrowRotation;
    if (compassHeading != null) {
      double turn = (bearingToTarget - compassHeading!) % 360;
      if (turn > 180) turn -= 360;
      if (turn.abs() <= 10) {
        directionText = l10n.navigationAhead;
      } else if (turn > 0) {
        directionText = l10n.navigationTurnRight(turn.round());
      } else {
        directionText = l10n.navigationTurnLeft(turn.abs().round());
      }
      arrowRotation = turn * (math.pi / 180);
    } else {
      directionText = '${_cardinal(bearingToTarget)} ${bearingToTarget.round()}°';
      arrowRotation = bearingToTarget * (math.pi / 180);
    }

    return AppPill(
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Transform.rotate(
            angle: arrowRotation,
            child: Icon(Icons.navigation, color: scheme.onSurfaceVariant),
          ),
          const SizedBox(width: AppSpacing.m),
          Text(distanceText,
              style: textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w500, color: scheme.onSurface)),
          const SizedBox(width: AppSpacing.s),
          Text(directionText,
              style: textTheme.bodySmall?.copyWith(color: scheme.onSurfaceVariant)),
          if (trailing != null) ...[
            const SizedBox(width: AppSpacing.s),
            trailing!,
          ],
          IconButton(
            visualDensity: VisualDensity.compact,
            onPressed: onDismiss,
            icon: Icon(Icons.close, size: 20, color: scheme.onSurfaceVariant),
          ),
        ],
      ),
    );
  }
}

/// Small inline compass heading, folded into the dominant chip instead of
/// stacking a second chip.
class _CompassBadge extends StatelessWidget {
  final double heading;
  final ColorScheme scheme;
  const _CompassBadge({required this.heading, required this.scheme});

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(Icons.explore, size: 16, color: scheme.onSurfaceVariant),
        const SizedBox(width: 2),
        Text('${_cardinal(heading)} ${heading.round()}°',
            style: Theme.of(context)
                .textTheme
                .labelSmall
                ?.copyWith(color: scheme.onSurfaceVariant)),
      ],
    );
  }
}

String _cardinal(double heading) {
  const dirs = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'];
  return dirs[((heading % 360 + 22.5) % 360 / 45).floor()];
}
