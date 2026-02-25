import 'package:flutter/material.dart';

/// Reusable footer row with text and a button link
class AuthFooterLink extends StatelessWidget {
  final String message;
  final String linkText;
  final VoidCallback onPressed;

  const AuthFooterLink({
    super.key,
    required this.message,
    required this.linkText,
    required this.onPressed,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return Center(
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            message,
            style: textTheme.bodySmall?.copyWith(
              color: colorScheme.onSurfaceVariant,
            ),
          ),
          TextButton(
            onPressed: onPressed,
            style: TextButton.styleFrom(
              foregroundColor: colorScheme.primary,
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
              textStyle: textTheme.labelMedium,
            ),
            child: Text(linkText),
          ),
        ],
      ),
    );
  }
}