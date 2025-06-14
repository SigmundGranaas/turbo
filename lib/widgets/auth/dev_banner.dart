import 'package:flutter/material.dart';
import 'package:turbo/l10n/app_localizations.dart';

/// A banner to indicate when the app is in development mode
class DevModeBanner extends StatelessWidget {
  const DevModeBanner({
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final l10n = context.l10n;

    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: Colors.amber.withOpacity(0.2),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: Colors.amber),
      ),
      child: Row(
        children: [
          Icon(Icons.info_outline, size: 20, color: Colors.amber.shade800),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              l10n.devMode,
              style: textTheme.bodyMedium?.copyWith(
                color: Colors.amber.shade800,
              ),
            ),
          ),
        ],
      ),
    );
  }
}