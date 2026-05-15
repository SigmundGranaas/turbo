import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';

TextTheme createTextTheme(BuildContext context, String fontString) {
  TextTheme baseTextTheme = Theme.of(context).textTheme;
  return GoogleFonts.getTextTheme(fontString, baseTextTheme);
}
