import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

// --- Theme Provider ---
final themeModeProvider = StateNotifierProvider<ThemeModeNotifier, ThemeMode>((ref) {
  return ThemeModeNotifier();
});

class ThemeModeNotifier extends StateNotifier<ThemeMode> {
  ThemeModeNotifier() : super(ThemeMode.system) {
    _loadThemeMode();
  }

  static const _themeModeKey = 'themeMode';

  Future<void> _loadThemeMode() async {
    final prefs = await SharedPreferences.getInstance();
    final themeModeString = prefs.getString(_themeModeKey);
    if (themeModeString == 'light') {
      state = ThemeMode.light;
    } else if (themeModeString == 'dark') {
      state = ThemeMode.dark;
    } else {
      state = ThemeMode.system;
    }
  }

  Future<void> setThemeMode(ThemeMode themeMode) async {
    if (!mounted) return;
    final prefs = await SharedPreferences.getInstance();
    state = themeMode;
    if (themeMode == ThemeMode.light) {
      await prefs.setString(_themeModeKey, 'light');
    } else if (themeMode == ThemeMode.dark) {
      await prefs.setString(_themeModeKey, 'dark');
    } else {
      await prefs.remove(_themeModeKey);
    }
  }
}

// --- Locale Provider ---
final localeProvider = StateNotifierProvider<LocaleNotifier, Locale>((ref) {
  return LocaleNotifier();
});

class LocaleNotifier extends StateNotifier<Locale> {
  LocaleNotifier() : super(const Locale('en')) { // Default to English
    _loadLocale();
  }

  static const _localeKey = 'locale';

  Future<void> _loadLocale() async {
    final prefs = await SharedPreferences.getInstance();
    final languageCode = prefs.getString(_localeKey);
    if (languageCode != null) {
      if (!mounted) return;
      state = Locale(languageCode);
    }
  }

  Future<void> setLocale(Locale locale) async {
    if (!mounted) return;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_localeKey, locale.languageCode);
    state = locale;
  }
}