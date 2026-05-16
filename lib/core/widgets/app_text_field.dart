import 'package:flutter/material.dart';

/// The single text field primitive. Visual style (filled, border radius,
/// padding, focus ring) comes from `inputDecorationTheme` set in `theme.dart`.
///
/// Replaces `AuthTextField` and every bare `TextFormField(decoration: ...)`
/// in feature widgets.
class AppTextField extends StatelessWidget {
  final TextEditingController? controller;
  final String label;
  final String? hintText;
  final TextInputType keyboardType;
  final bool obscureText;
  final Widget? suffixIcon;
  final String? Function(String?)? validator;
  final int? maxLines;
  final bool autofocus;
  final void Function(String)? onChanged;

  const AppTextField({
    super.key,
    this.controller,
    required this.label,
    this.hintText,
    this.keyboardType = TextInputType.text,
    this.obscureText = false,
    this.suffixIcon,
    this.validator,
    this.maxLines = 1,
    this.autofocus = false,
    this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return TextFormField(
      controller: controller,
      keyboardType: keyboardType,
      obscureText: obscureText,
      validator: validator,
      maxLines: obscureText ? 1 : maxLines,
      autofocus: autofocus,
      onChanged: onChanged,
      decoration: InputDecoration(
        labelText: label,
        hintText: hintText,
        suffixIcon: suffixIcon,
      ),
    );
  }
}
