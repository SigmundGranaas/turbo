import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_grouped_card.dart';
import 'package:turbo/core/widgets/app_section_header.dart';

class AboutSettingsPage extends StatelessWidget {
  const AboutSettingsPage({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('About')),
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 700),
          child: ListView(
            padding: const EdgeInsets.fromLTRB(
                AppSpacing.l, AppSpacing.xs, AppSpacing.l, AppSpacing.xl),
            children: [
              const _AppBanner(),
              const SizedBox(height: AppSpacing.l),
              const AppSectionHeader('App'),
              FutureBuilder<PackageInfo>(
                future: PackageInfo.fromPlatform(),
                builder: (context, snapshot) {
                  final version = snapshot.hasData
                      ? '${snapshot.data!.version} (build ${snapshot.data!.buildNumber})'
                      : 'Loading…';
                  return AppGroupedCard(
                    child: Column(
                      children: [
                        _AboutRow(
                          icon: Icons.verified_outlined,
                          title: 'Version',
                          subtitle: version,
                        ),
                        const _RowDivider(),
                        _AboutRow(
                          icon: Icons.layers_outlined,
                          title: 'Tile sources',
                          subtitle: 'Norgeskart · OSM · Google Sat.',
                        ),
                        const _RowDivider(),
                        _AboutRow(
                          icon: Icons.description_outlined,
                          title: 'Open-source licenses',
                          trailingChevron: true,
                          onTap: () => showLicensePage(
                            context: context,
                            applicationName: 'Turbo',
                            applicationVersion: snapshot.data?.version,
                          ),
                        ),
                      ],
                    ),
                  );
                },
              ),
              const SizedBox(height: AppSpacing.l),
              const AppSectionHeader('Legal'),
              AppGroupedCard(
                child: Column(
                  children: const [
                    _AboutRow(
                      icon: Icons.shield_outlined,
                      title: 'Privacy policy',
                      trailingExternal: true,
                    ),
                    _RowDivider(),
                    _AboutRow(
                      icon: Icons.gavel_outlined,
                      title: 'Terms of use',
                      trailingExternal: true,
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _AppBanner extends StatelessWidget {
  const _AppBanner();

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(
          horizontal: AppSpacing.l + 2, vertical: AppSpacing.xl - 4),
      decoration: BoxDecoration(
        color: colorScheme.primaryContainer,
        borderRadius: BorderRadius.circular(28),
      ),
      child: Row(
        children: [
          Container(
            width: 56,
            height: 56,
            decoration: BoxDecoration(
              color: colorScheme.surface,
              shape: BoxShape.circle,
            ),
            alignment: Alignment.center,
            child: Text(
              'T',
              style: TextStyle(
                color: colorScheme.primary,
                fontSize: 24,
                height: 28 / 24,
                fontWeight: FontWeight.w700,
              ),
            ),
          ),
          const SizedBox(width: 14),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'Turbo',
                  style: TextStyle(
                    color: colorScheme.onPrimaryContainer,
                    fontSize: 22,
                    height: 28 / 22,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  'Turkart — hiking maps for Norway',
                  style: TextStyle(
                    color: colorScheme.onPrimaryContainer
                        .withValues(alpha: 0.85),
                    fontSize: 13,
                    height: 18 / 13,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _AboutRow extends StatelessWidget {
  final IconData icon;
  final String title;
  final String? subtitle;
  final bool trailingChevron;
  final bool trailingExternal;
  final VoidCallback? onTap;

  const _AboutRow({
    required this.icon,
    required this.title,
    this.subtitle,
    this.trailingChevron = false,
    this.trailingExternal = false,
    this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final trailing = trailingExternal
        ? Icons.open_in_new
        : (trailingChevron ? Icons.chevron_right : null);
    return InkWell(
      onTap: onTap,
      onLongPress: subtitle != null
          ? () => Clipboard.setData(ClipboardData(text: subtitle!))
          : null,
      child: Padding(
        padding: const EdgeInsets.symmetric(
            horizontal: AppSpacing.l, vertical: 14),
        child: Row(
          children: [
            Icon(icon, size: 20, color: colorScheme.onSurfaceVariant),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: TextStyle(
                      color: colorScheme.onSurface,
                      fontSize: 15,
                      height: 20 / 15,
                    ),
                  ),
                  if (subtitle != null) ...[
                    const SizedBox(height: 2),
                    Text(
                      subtitle!,
                      style: TextStyle(
                        color: colorScheme.onSurfaceVariant,
                        fontSize: 12,
                        height: 16 / 12,
                      ),
                    ),
                  ],
                ],
              ),
            ),
            if (trailing != null)
              Icon(trailing, size: 18, color: colorScheme.onSurfaceVariant),
          ],
        ),
      ),
    );
  }
}

class _RowDivider extends StatelessWidget {
  const _RowDivider();

  @override
  Widget build(BuildContext context) {
    return Divider(
      height: 1,
      thickness: 1,
      indent: AppSpacing.l,
      endIndent: AppSpacing.l,
      color: Theme.of(context).colorScheme.outlineVariant,
    );
  }
}
