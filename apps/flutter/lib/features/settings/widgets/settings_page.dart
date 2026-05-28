import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/location/gps_accuracy_mode.dart';
import 'package:turbo/core/util/distance_formatter.dart';
import 'package:turbo/features/auth/api.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';
import 'package:turbo/features/settings/widgets/sections/about_settings_page.dart';
import 'package:turbo/features/settings/widgets/sections/appearance_settings_page.dart';
import 'package:turbo/features/settings/widgets/sections/drawing_settings_page.dart';
import 'package:turbo/features/settings/widgets/sections/location_marker_settings_page.dart';
import 'package:turbo/features/settings/widgets/sections/recording_settings_page.dart';
import 'package:turbo/features/settings/widgets/sections/units_settings_page.dart';

class SettingsPage extends ConsumerWidget {
  const SettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final settingsAsync = ref.watch(settingsProvider);

    return Scaffold(
      appBar: AppBar(title: Text(l10n.settings)),
      body: settingsAsync.when(
        data: (settings) => Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 700),
            child: ListView(
              padding: const EdgeInsets.only(top: AppSpacing.xs, bottom: AppSpacing.xl),
              children: [
                const _AccountCard(),
                const _LandingLabel('App'),
                _LandingCard(children: [
                  _CategoryRow(
                    icon: Icons.palette_outlined,
                    title: 'Appearance',
                    subtitle: _appearanceSummary(settings, l10n),
                    builder: (_) => const AppearanceSettingsPage(),
                  ),
                  _CategoryRow(
                    icon: Icons.gesture_outlined,
                    title: l10n.drawing,
                    subtitle: _drawingSummary(settings, l10n),
                    builder: (_) => const DrawingSettingsPage(),
                  ),
                  _CategoryRow(
                    icon: Icons.my_location_outlined,
                    title: l10n.myLocation,
                    subtitle: _locationSummary(settings),
                    builder: (_) => const LocationMarkerSettingsPage(),
                  ),
                ]),
                const _LandingLabel('Recording & system'),
                _LandingCard(children: [
                  _CategoryRow(
                    icon: Icons.fiber_smart_record_outlined,
                    title: 'Recording',
                    subtitle: _recordingSummary(settings),
                    builder: (_) => const RecordingSettingsPage(),
                  ),
                  _CategoryRow(
                    icon: Icons.straighten_outlined,
                    title: 'Units & downloads',
                    subtitle: _unitsSummary(settings, l10n),
                    builder: (_) => const UnitsSettingsPage(),
                  ),
                  _CategoryRow(
                    icon: Icons.info_outline,
                    title: 'About Turbo',
                    subtitle: 'Version, licenses, legal',
                    builder: (_) => const AboutSettingsPage(),
                  ),
                ]),
              ],
            ),
          ),
        ),
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (_, _) => Center(child: Text(l10n.genericLoadError)),
      ),
    );
  }

  String _appearanceSummary(SettingsState s, AppLocalizations l10n) {
    final theme = switch (s.themeMode) {
      ThemeMode.light => l10n.light,
      ThemeMode.dark => l10n.dark,
      ThemeMode.system => l10n.system,
    };
    final lang = s.locale.languageCode == 'nb' ? l10n.norwegian : l10n.english;
    return '$theme · $lang';
  }

  String _drawingSummary(SettingsState s, AppLocalizations l10n) {
    final smooth = s.smoothLine ? l10n.smoothLine : 'Straight line';
    return '$smooth · ${s.drawSensitivity.round()} px';
  }

  String _locationSummary(SettingsState s) {
    final iconLabel = switch (s.locationIconType) {
      'builtin' => 'Built-in icon',
      'custom' => 'Custom image',
      _ => 'Default dot',
    };
    return '$iconLabel · ${s.locationMarkerSize.toStringAsFixed(1)}×';
  }

  String _recordingSummary(SettingsState s) {
    final gps = switch (s.gpsAccuracyMode) {
      GpsAccuracyMode.high => 'High GPS',
      GpsAccuracyMode.balanced => 'Balanced GPS',
      GpsAccuracyMode.batterySaver => 'Battery saver',
    };
    final screen = s.keepScreenOnWhileRecording ? 'Keep screen on' : 'Screen sleeps';
    return '$gps · $screen';
  }

  String _unitsSummary(SettingsState s, AppLocalizations l10n) {
    final unit = s.distanceUnit == DistanceUnit.metric
        ? l10n.distanceUnitMetric
        : l10n.distanceUnitImperial;
    return '$unit · ${s.maxConcurrentDownloads} parallel downloads';
  }
}

/// Pixel-style account row. Authenticated → email + tap to open profile.
/// Unauthenticated → "Sign in" call to action.
class _AccountCard extends ConsumerWidget {
  const _AccountCard();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final colorScheme = Theme.of(context).colorScheme;
    final auth = ref.watch(authStateProvider);
    final authed = auth.status == AuthStatus.authenticated;
    final email = auth.email;
    final initials = _initials(email);

    return Padding(
      padding: const EdgeInsets.fromLTRB(
          AppSpacing.l, AppSpacing.xs, AppSpacing.l, 0),
      child: Material(
        color: colorScheme.primaryContainer,
        borderRadius: BorderRadius.circular(28),
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          onTap: authed
              ? () => Navigator.of(context).push(MaterialPageRoute(
                    builder: (_) => const UserProfileScreen(),
                  ))
              : null,
          child: Padding(
            padding: const EdgeInsets.symmetric(
                horizontal: AppSpacing.l, vertical: 14),
            child: Row(
              children: [
                Container(
                  width: 44,
                  height: 44,
                  decoration: BoxDecoration(
                    color: colorScheme.surface,
                    shape: BoxShape.circle,
                  ),
                  alignment: Alignment.center,
                  child: authed
                      ? Text(
                          initials,
                          style: TextStyle(
                            color: colorScheme.primary,
                            fontSize: 18,
                            fontWeight: FontWeight.w500,
                          ),
                        )
                      : Icon(
                          Icons.person_outline,
                          color: colorScheme.primary,
                          size: 22,
                        ),
                ),
                const SizedBox(width: 14),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        authed ? (email ?? 'Signed in') : 'Sign in',
                        style: TextStyle(
                          color: colorScheme.onPrimaryContainer,
                          fontSize: 15,
                          height: 20 / 15,
                          fontWeight: FontWeight.w500,
                        ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                      const SizedBox(height: 2),
                      Text(
                        authed
                            ? 'Manage your Turbo account'
                            : 'Sync markers and paths across devices',
                        style: TextStyle(
                          color: colorScheme.onPrimaryContainer
                              .withValues(alpha: 0.85),
                          fontSize: 12,
                          height: 16 / 12,
                        ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                  ),
                ),
                if (authed)
                  Icon(
                    Icons.chevron_right,
                    color: colorScheme.onPrimaryContainer,
                    size: 22,
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  String _initials(String? email) {
    if (email == null || email.isEmpty) return '?';
    final at = email.indexOf('@');
    final local = at > 0 ? email.substring(0, at) : email;
    final parts = local.split(RegExp(r'[._\-+]'));
    final letters = parts
        .where((p) => p.isNotEmpty)
        .map((p) => p[0].toUpperCase())
        .take(2)
        .join();
    return letters.isEmpty ? local[0].toUpperCase() : letters;
  }
}

/// Primary-tinted titleSmall label rendered between landing cards.
class _LandingLabel extends StatelessWidget {
  final String text;
  const _LandingLabel(this.text);

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(
          AppSpacing.l + AppSpacing.xs, AppSpacing.l, AppSpacing.l, AppSpacing.s),
      child: Text(
        text,
        style: TextStyle(
          color: Theme.of(context).colorScheme.primary,
          fontSize: 13,
          height: 20 / 13,
          fontWeight: FontWeight.w500,
          letterSpacing: 0.4,
        ),
      ),
    );
  }
}

/// 28px-radius grouped card used for category lists on the landing page.
/// Distinct from the 16px `AppGroupedCard` used inside category pages.
class _LandingCard extends StatelessWidget {
  final List<Widget> children;
  const _LandingCard({required this.children});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.l),
      child: Material(
        color: Theme.of(context).colorScheme.surfaceContainer,
        borderRadius: BorderRadius.circular(28),
        clipBehavior: Clip.antiAlias,
        child: Column(children: children),
      ),
    );
  }
}

/// Pixel/Android-14+ settings row: bare icon, title + subtitle, no chevron.
class _CategoryRow extends StatelessWidget {
  final IconData icon;
  final String title;
  final String subtitle;
  final WidgetBuilder builder;

  const _CategoryRow({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.builder,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    return InkWell(
      onTap: () => Navigator.of(context)
          .push(MaterialPageRoute(builder: builder)),
      child: Padding(
        padding: const EdgeInsets.symmetric(
            horizontal: AppSpacing.xl - 4, vertical: 14),
        child: Row(
          children: [
            Icon(icon, size: 24, color: colorScheme.onSurfaceVariant),
            const SizedBox(width: 20),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: TextStyle(
                      color: colorScheme.onSurface,
                      fontSize: 16,
                      height: 22 / 16,
                      letterSpacing: 0.15,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    subtitle,
                    style: TextStyle(
                      color: colorScheme.onSurfaceVariant,
                      fontSize: 13,
                      height: 18 / 13,
                      letterSpacing: 0.25,
                    ),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
