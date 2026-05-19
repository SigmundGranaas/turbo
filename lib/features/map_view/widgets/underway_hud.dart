import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/location/compass_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/core/widgets/app_pill.dart';
import 'package:turbo/features/settings/api.dart';

/// Marine-style HUD pill that surfaces course over ground (COG, from GPS),
/// speed over ground (SOG, from GPS), and magnetic heading (HDG, from the
/// compass). Mirrors what a small chart plotter shows so the user can keep
/// one eye on the boat while panning the map.
///
/// Reads the same GPS stream as everything else via [lastPositionProvider];
/// no extra platform subscription is opened.
class UnderwayHud extends ConsumerWidget {
  const UnderwayHud({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final enabled = ref.watch(settingsProvider
        .select((s) => s.value?.showUnderwayHud ?? false));
    if (!enabled) return const SizedBox.shrink();

    final snapshot = ref.watch(lastPositionProvider);
    final unit = ref.watch(settingsProvider
        .select((s) => s.value?.distanceUnit ?? DistanceUnit.metric));
    final compassHeading = ref.watch(compassStateProvider).value;
    final l10n = AppLocalizations.of(context);

    return AppPill(
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.l,
        vertical: AppSpacing.s,
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          _HudField(
            label: l10n.hudLabelSog,
            value: snapshot?.speedMps == null
                ? '—'
                : formatSpeed(snapshot!.speedMps!, unit),
          ),
          const SizedBox(width: AppSpacing.l),
          _HudField(
            label: l10n.hudLabelCog,
            value: snapshot?.courseDeg == null
                ? '—'
                : _formatBearing(snapshot!.courseDeg!),
          ),
          const SizedBox(width: AppSpacing.l),
          _HudField(
            label: l10n.hudLabelHdg,
            value: compassHeading == null
                ? '—'
                : _formatBearing(compassHeading),
          ),
        ],
      ),
    );
  }

  String _formatBearing(double deg) {
    // Normalize and zero-pad to three digits — standard nautical convention.
    final n = ((deg % 360) + 360) % 360;
    return '${n.round().toString().padLeft(3, '0')}°';
  }
}

class _HudField extends StatelessWidget {
  final String label;
  final String value;

  const _HudField({required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        Text(
          label,
          style: textTheme.labelSmall?.copyWith(
            color: colorScheme.onSurfaceVariant,
            letterSpacing: 0.8,
          ),
        ),
        Text(
          value,
          style: textTheme.titleSmall?.copyWith(
            fontFeatures: const [FontFeature.tabularFigures()],
            fontWeight: FontWeight.w600,
          ),
        ),
      ],
    );
  }
}
