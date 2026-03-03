import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart' hide DistanceCalculator;

import 'package:turbo/core/location/compass_mode_state.dart';
import 'package:turbo/core/location/compass_state.dart';
import 'package:turbo/core/location/follow_mode_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/features/navigation/data/navigation_state_notifier.dart';
import 'package:turbo/l10n/app_localizations.dart';

class ModeIndicator extends ConsumerWidget {
  const ModeIndicator({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final isFollowing = ref.watch(followModeProvider);
    final isCompassMode = ref.watch(compassModeProvider);
    final compassHeading = ref.watch(compassStateProvider).value;
    final navState = ref.watch(navigationStateProvider);

    final showModeChips = isFollowing || isCompassMode;
    final showNavChip = navState.isActive && navState.target != null;

    if (!showModeChips && !showNavChip) {
      return const SizedBox.shrink();
    }

    final colorScheme = Theme.of(context).colorScheme;
    final l10n = context.l10n;

    return Positioned(
      bottom: 16,
      left: 16,
      right: 16,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (showNavChip)
            _NavigationInfoChip(
              target: navState.target!,
              compassHeading: compassHeading,
              colorScheme: colorScheme,
            ),
          if (showNavChip && showModeChips)
            const SizedBox(height: 8),
          if (showModeChips)
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                if (isFollowing)
                  _ModeChip(
                    icon: Icons.my_location,
                    label: l10n.following,
                    colorScheme: colorScheme,
                    onDismiss: () =>
                        ref.read(followModeProvider.notifier).disable(),
                  ),
                if (isFollowing && isCompassMode)
                  const SizedBox(width: 8),
                if (isCompassMode)
                  _ModeChip(
                    icon: Icons.explore,
                    label: compassHeading != null
                        ? '${_headingToCardinal(compassHeading)} ${compassHeading.round()}°'
                        : l10n.compassOrientation,
                    colorScheme: colorScheme,
                    onDismiss: () =>
                        ref.read(compassModeProvider.notifier).disable(),
                  ),
              ],
            ),
        ],
      ),
    );
  }

  static String _headingToCardinal(double heading) {
    const directions = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'];
    final index = ((heading + 22.5) % 360 / 45).floor();
    return directions[index];
  }
}

class _NavigationInfoChip extends ConsumerWidget {
  final LatLng target;
  final double? compassHeading;
  final ColorScheme colorScheme;

  const _NavigationInfoChip({
    required this.target,
    required this.compassHeading,
    required this.colorScheme,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final locationAsync = ref.watch(locationStateProvider);
    final userPosition = locationAsync.value;
    final textTheme = Theme.of(context).textTheme;
    final l10n = context.l10n;

    if (userPosition == null) return const SizedBox.shrink();

    final distanceM = const Distance().distance(userPosition, target);
    final distanceText = distanceM >= 1000
        ? '${(distanceM / 1000).toStringAsFixed(2)} km'
        : '${distanceM.round()} m';

    final bearingToTarget = const Distance().bearing(userPosition, target);

    // Compute turn angle and direction label
    String directionText;
    double arrowRotation; // radians for the arrow icon

    if (compassHeading != null) {
      double turnAngle = (bearingToTarget - compassHeading!) % 360;
      if (turnAngle > 180) turnAngle -= 360;
      // turnAngle: positive = turn right, negative = turn left

      if (turnAngle.abs() <= 10) {
        directionText = l10n.navigationAhead;
      } else if (turnAngle > 0) {
        directionText = l10n.navigationTurnRight(turnAngle.round());
      } else {
        directionText = l10n.navigationTurnLeft(turnAngle.abs().round());
      }

      // Arrow points toward target relative to current heading
      arrowRotation = turnAngle * (math.pi / 180);
    } else {
      // No compass — show absolute bearing
      directionText = '${_headingToCardinal(bearingToTarget)} ${bearingToTarget.round()}°';
      arrowRotation = bearingToTarget * (math.pi / 180);
    }

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      decoration: BoxDecoration(
        color: colorScheme.tertiaryContainer,
        borderRadius: BorderRadius.circular(20),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.15),
            blurRadius: 4,
            offset: const Offset(0, 2),
          ),
        ],
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Transform.rotate(
            angle: arrowRotation,
            child: Icon(
              Icons.navigation,
              size: 16,
              color: colorScheme.onTertiaryContainer,
            ),
          ),
          const SizedBox(width: 6),
          Text(
            distanceText,
            style: textTheme.labelMedium?.copyWith(
              fontWeight: FontWeight.w500,
              color: colorScheme.onTertiaryContainer,
            ),
          ),
          const SizedBox(width: 6),
          Text(
            directionText,
            style: textTheme.labelMedium?.copyWith(
              color: colorScheme.onTertiaryContainer,
            ),
          ),
          const SizedBox(width: 6),
          GestureDetector(
            onTap: () => ref.read(navigationStateProvider.notifier).stopNavigation(),
            behavior: HitTestBehavior.opaque,
            child: Padding(
              padding: const EdgeInsets.all(2),
              child: Icon(
                Icons.close,
                size: 16,
                color: colorScheme.onTertiaryContainer,
              ),
            ),
          ),
        ],
      ),
    );
  }

  static String _headingToCardinal(double heading) {
    const directions = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'];
    final index = ((heading % 360 + 22.5) % 360 / 45).floor();
    return directions[index];
  }
}

class _ModeChip extends StatelessWidget {
  final IconData icon;
  final String label;
  final ColorScheme colorScheme;
  final VoidCallback onDismiss;

  const _ModeChip({
    required this.icon,
    required this.label,
    required this.colorScheme,
    required this.onDismiss,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      decoration: BoxDecoration(
        color: colorScheme.tertiaryContainer,
        borderRadius: BorderRadius.circular(20),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.15),
            blurRadius: 4,
            offset: const Offset(0, 2),
          ),
        ],
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            icon,
            size: 16,
            color: colorScheme.onTertiaryContainer,
          ),
          const SizedBox(width: 6),
          Text(
            label,
            style: textTheme.labelMedium?.copyWith(
              fontWeight: FontWeight.w500,
              color: colorScheme.onTertiaryContainer,
            ),
          ),
          const SizedBox(width: 6),
          GestureDetector(
            onTap: onDismiss,
            behavior: HitTestBehavior.opaque,
            child: Padding(
              padding: const EdgeInsets.all(2),
              child: Icon(
                Icons.close,
                size: 16,
                color: colorScheme.onTertiaryContainer,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
