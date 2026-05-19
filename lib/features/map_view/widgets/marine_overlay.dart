import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/map_view/widgets/underway_hud.dart';
import 'package:turbo/features/settings/api.dart';
import 'package:turbo/features/weather/api.dart' show MarineWindStrip;

/// Bottom-of-map stack for the optional marine widgets (wind strip + underway
/// HUD). Lives here rather than inside each widget so the inter-pill spacing
/// only appears when both pills are actually visible — preventing the stack
/// from drifting up by 8 px when only one is enabled.
class MarineOverlay extends ConsumerWidget {
  const MarineOverlay({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final hudOn = ref.watch(settingsProvider
        .select((s) => s.value?.showUnderwayHud ?? false));
    final windOn = ref.watch(settingsProvider
        .select((s) => s.value?.showWindStrip ?? false));

    if (!hudOn && !windOn) return const SizedBox.shrink();

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (windOn) const MarineWindStrip(),
        if (windOn && hudOn) const SizedBox(height: 8),
        if (hudOn) const UnderwayHud(),
      ],
    );
  }
}
