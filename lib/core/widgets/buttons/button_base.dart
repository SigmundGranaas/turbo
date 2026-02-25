import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';

/// A base class for button styling and behavior
/// that can be extended by specific button types
abstract class ButtonBase extends StatelessWidget {
  final String text;
  final VoidCallback? onPressed;
  final bool isLoading;
  final EdgeInsetsGeometry? padding;

  const ButtonBase({
    super.key,
    required this.text,
    this.onPressed,
    this.isLoading = false,
    this.padding,
  });

  // Default text style used by all buttons
  TextStyle get textStyle => GoogleFonts.roboto(
    fontSize: 16,
    fontWeight: FontWeight.w500,
    letterSpacing: 0.5,
  );

  // Default padding used by all buttons
  EdgeInsetsGeometry get defaultPadding =>
      padding ?? const EdgeInsets.symmetric(vertical: 20, horizontal: 24);

  // Default border radius used by all buttons
  BorderRadius get borderRadius => BorderRadius.circular(28);

  // The loading indicator - can be overridden by subclasses
  Widget buildLoadingIndicator(Color color) {
    return SizedBox(
      height: 20,
      width: 20,
      child: CircularProgressIndicator(
        strokeWidth: 2,
        color: color,
      ),
    );
  }

  // Abstract method to be implemented by subclasses
  Widget buildButton(BuildContext context);

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: double.infinity,
      child: buildButton(context),
    );
  }
}