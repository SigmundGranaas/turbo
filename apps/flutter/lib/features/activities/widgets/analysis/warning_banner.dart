import 'package:flutter/material.dart';

import '../../models/activity_analysis.dart';

/// Severity-tinted banner for a single warning. The detail screen sorts
/// warnings by severity and renders one banner per warning above the
/// driver list — never collapsed, because the entire point of a warning
/// is that the user sees it before deciding to go.
class WarningBanner extends StatelessWidget {
  final AnalysisWarning warning;
  const WarningBanner({super.key, required this.warning});

  @override
  Widget build(BuildContext context) {
    final (color, icon) = switch (warning.severity) {
      WarningSeverity.danger => (Colors.red.shade700, Icons.warning_amber_rounded),
      WarningSeverity.caution => (Colors.orange.shade700, Icons.error_outline),
      WarningSeverity.info => (Colors.blue.shade700, Icons.info_outline),
    };
    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 12),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.10),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, color: color),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  warning.title,
                  style: TextStyle(
                    color: color,
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 2),
                Text(warning.body, style: Theme.of(context).textTheme.bodySmall),
                if (warning.sourceUrl != null) ...[
                  const SizedBox(height: 4),
                  Text(
                    'Source: ${warning.sourceUrl!}',
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                          color: color.withValues(alpha: 0.8),
                          decoration: TextDecoration.underline,
                        ),
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}
