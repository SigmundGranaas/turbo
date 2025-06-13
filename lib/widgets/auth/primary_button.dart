import 'package:flutter/material.dart';
import 'button_base.dart';

class PrimaryButton extends ButtonBase {
  const PrimaryButton({
    super.key,
    required super.text,
    super.onPressed,
    super.isLoading = false,
  });

  @override
  EdgeInsetsGeometry get defaultPadding =>
      const EdgeInsets.symmetric(vertical: 12, horizontal: 24);

  @override
  Widget buildButton(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return FilledButton(
      onPressed: isLoading || onPressed == null ? null : onPressed,
      style: FilledButton.styleFrom(
        padding: defaultPadding,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(100),
        ),
      ),
      child: isLoading
          ? buildLoadingIndicator(colorScheme.onPrimary)
          : Text(
        text,
        style: textTheme.labelLarge?.copyWith(
          color: colorScheme.onPrimary,
          fontWeight: FontWeight.bold,
        ),
      ),
    );
  }
}