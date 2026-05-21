import 'package:flutter/material.dart';

import '../models/met_alert.dart';

/// Compact warning chip rendered above the marker weather summary. Tap to
/// expand the description in a dialog.
class MetAlertBanner extends StatelessWidget {
  final MetAlert alert;
  const MetAlertBanner({super.key, required this.alert});

  @override
  Widget build(BuildContext context) {
    final scheme = _levelColors(context, alert.level);
    return Material(
      color: scheme.background,
      borderRadius: BorderRadius.circular(12),
      child: InkWell(
        key: const Key('met-alert-banner'),
        borderRadius: BorderRadius.circular(12),
        onTap: () => _showDetail(context),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          child: Row(
            children: [
              Icon(Icons.warning_amber_rounded, color: scheme.foreground),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      alert.event,
                      style: Theme.of(context).textTheme.titleSmall?.copyWith(
                            color: scheme.foreground,
                            fontWeight: FontWeight.w600,
                          ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    Text(
                      alert.description,
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                            color: scheme.foreground.withValues(alpha: 0.85),
                          ),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
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

  void _showDetail(BuildContext context) {
    showDialog<void>(
      context: context,
      builder: (_) => AlertDialog(
        title: Text(alert.event),
        content: SingleChildScrollView(
          child: Text(alert.description),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: Text(MaterialLocalizations.of(context).closeButtonLabel),
          ),
        ],
      ),
    );
  }

  static _LevelColors _levelColors(BuildContext context, MetAlertLevel level) {
    final scheme = Theme.of(context).colorScheme;
    switch (level) {
      case MetAlertLevel.yellow:
        return _LevelColors(
          background: const Color(0xFFFFF6D9),
          foreground: const Color(0xFF6B5300),
        );
      case MetAlertLevel.orange:
        return _LevelColors(
          background: const Color(0xFFFFE0CC),
          foreground: const Color(0xFF7A3B00),
        );
      case MetAlertLevel.red:
        return _LevelColors(
          background: scheme.errorContainer,
          foreground: scheme.onErrorContainer,
        );
    }
  }
}

class _LevelColors {
  final Color background;
  final Color foreground;
  const _LevelColors({required this.background, required this.foreground});
}
