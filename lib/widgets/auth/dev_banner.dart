import 'package:flutter/material.dart';

/// A banner to indicate when the app is in development mode
class DevModeBanner extends StatelessWidget {
  final String message;

  const DevModeBanner({
    super.key,
    this.message = 'Development mode',
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;

    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: Colors.amber.withValues(alpha: 0.2),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: Colors.amber),
      ),
      child: Row(
        children: [
          Icon(Icons.info_outline, size: 20, color: Colors.amber.shade800),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              message,
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