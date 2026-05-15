import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:google_fonts/google_fonts.dart';

class LoadingNotifier extends Notifier<bool> {
  @override
  bool build() => false;
  void set(bool value) => state = value;
}

TextTheme createTextTheme(BuildContext context, String fontString) {
  TextTheme baseTextTheme = Theme.of(context).textTheme;
  return GoogleFonts.getTextTheme(fontString, baseTextTheme);
}