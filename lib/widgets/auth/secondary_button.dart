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
  Widget buildButton(BuildContext context) {
    final buttonStyle = TextButton.styleFrom(
      foregroundColor: Colors.blue.shade700,
      padding: defaultPadding,
      shape: RoundedRectangleBorder(
        borderRadius: borderRadius,
      ),
    );

    Widget buttonChild = isLoading
        ? buildLoadingIndicator(Colors.blue.shade700)
        : Text(text, style: textStyle);

    return TextButton(
      onPressed: isLoading ? null : onPressed,
      style: buttonStyle,
      child: buttonChild,
    );
  }
}