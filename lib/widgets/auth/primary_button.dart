import 'package:flutter/material.dart';
import 'button_base.dart';

class PrimaryButton extends ButtonBase {
  const PrimaryButton({
    super.key,
    required super.text,
    super.onPressed,
    super.isLoading = false,
    super.padding,
  });

  @override
  Widget buildButton(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    final buttonStyle = ElevatedButton.styleFrom(
      foregroundColor: colorScheme.onPrimary,
      backgroundColor: colorScheme.primary,
      elevation: 0,
      padding: defaultPadding,
      shape: RoundedRectangleBorder(
        borderRadius: borderRadius,
      ),
    );

    Widget buttonChild = isLoading
        ? buildLoadingIndicator(colorScheme.onPrimary)
        : Text(
      text,
      style: textTheme.labelLarge?.copyWith(
        color: colorScheme.onPrimary,
        fontWeight: FontWeight.w500,
        letterSpacing: 0.5,
      ),
    );

    return ElevatedButton(
      onPressed: isLoading ? null : onPressed,
      style: buttonStyle,
      child: buttonChild,
    );
  }
}