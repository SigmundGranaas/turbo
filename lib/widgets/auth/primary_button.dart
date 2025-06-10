import 'package:flutter/material.dart';
import 'button_base.dart';

class PrimaryButton extends ButtonBase {
  const PrimaryButton({
    super.key,
    required super.text,
    super.onPressed,
    super.isLoading = false,
  });

  // Use a slightly smaller vertical padding for a less tall button.
  @override
  EdgeInsetsGeometry get defaultPadding =>
      const EdgeInsets.symmetric(vertical: 16, horizontal: 24);

  @override
  Widget buildButton(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return ElevatedButton(
      onPressed: isLoading || onPressed == null ? null : onPressed,
      style: ElevatedButton.styleFrom(
        foregroundColor: colorScheme.onPrimary,
        backgroundColor: colorScheme.primary,
        disabledForegroundColor: colorScheme.onPrimary.withOpacity(0.5),
        disabledBackgroundColor: colorScheme.primary.withOpacity(0.7),
        elevation: 0,
        padding: defaultPadding,
        shape: RoundedRectangleBorder(
          borderRadius: borderRadius,
        ),
      ),
      child: isLoading
          ? buildLoadingIndicator(colorScheme.onPrimary)
          : Text(
        text,
        style: textTheme.labelLarge?.copyWith(
          color: colorScheme.onPrimary,
          fontWeight: FontWeight.w500,
          letterSpacing: 0.5,
        ),
      ),
    );
  }
}