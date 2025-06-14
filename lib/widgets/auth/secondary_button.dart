import 'package:flutter/material.dart';
import 'button_base.dart';

class SecondaryButton extends ButtonBase {
  const SecondaryButton({
    super.key,
    required super.text,
    super.onPressed,
    super.isLoading = false,
    super.padding,
  });

  @override
  EdgeInsetsGeometry get defaultPadding =>
      const EdgeInsets.symmetric(vertical: 24, horizontal: 24);

  @override
  Widget buildButton(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return OutlinedButton(
      onPressed: isLoading || onPressed == null ? null : onPressed,
      style: OutlinedButton.styleFrom(
        padding: defaultPadding,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(100),
        ),
        side: BorderSide(color: colorScheme.outline),
      ),
      child: isLoading
          ? buildLoadingIndicator(colorScheme.primary)
          : Text(
        text,
        style: textTheme.labelLarge?.copyWith(
          color: colorScheme.primary,
          fontWeight: FontWeight.bold,
        ),
      ),
    );
  }
}