import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Defines the state for all user-configurable settings.
@immutable
class SettingsState {
  final ThemeMode themeMode;
  final Locale locale;

  const SettingsState({
    required this.themeMode,
    required this.locale,
  });

  // Default initial state
  factory SettingsState.initial() => const SettingsState(
    themeMode: ThemeMode.system,
    locale: Locale('en'),
  );

  SettingsState copyWith({
    ThemeMode? themeMode,
    Locale? locale,
  }) {
    return SettingsState(
      themeMode: themeMode ?? this.themeMode,
      locale: locale ?? this.locale,
    );
  }
}

/// Manages loading, updating, and persisting user settings.
///
/// This is an AsyncNotifier because its initialization (loading from
/// SharedPreferences) is an asynchronous operation.
class SettingsNotifier extends AsyncNotifier<SettingsState> {
  static const _themeModeKey = 'themeMode';
  static const _localeKey = 'locale';

  @override
  Future<SettingsState> build() async {
    return _loadSettings();
  }

  Future<SettingsState> _loadSettings() async {
    final prefs = await SharedPreferences.getInstance();

    // Load Theme
    final themeModeString = prefs.getString(_themeModeKey);
    ThemeMode themeMode;
    if (themeModeString == 'light') {
      themeMode = ThemeMode.light;
    } else if (themeModeString == 'dark') {
      themeMode = ThemeMode.dark;
    } else {
      themeMode = ThemeMode.system;
    }

    // Load Locale
    final languageCode = prefs.getString(_localeKey);
    final locale = languageCode != null ? Locale(languageCode) : const Locale('en');

    return SettingsState(themeMode: themeMode, locale: locale);
  }

  /// Updates the theme mode and persists it to local storage.
  Future<void> setThemeMode(ThemeMode newThemeMode) async {
    // We only update the state if the build has completed.
    if (state.value == null) return;

    // Update the state immediately for a responsive UI.
    state = AsyncData(state.value!.copyWith(themeMode: newThemeMode));

    // Persist the change.
    final prefs = await SharedPreferences.getInstance();
    if (newThemeMode == ThemeMode.system) {
      await prefs.remove(_themeModeKey);
    } else {
      await prefs.setString(_themeModeKey, newThemeMode.name);
    }
  }

  /// Updates the application locale and persists it to local storage.
  Future<void> setLocale(Locale newLocale) async {
    if (state.value == null) return;

    // Update the state.
    state = AsyncData(state.value!.copyWith(locale: newLocale));

    // Persist the change.
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_localeKey, newLocale.languageCode);
  }
}

/// The provider for accessing the SettingsNotifier and its state.
///
/// This is the single source of truth for all user settings.
final settingsProvider = AsyncNotifierProvider<SettingsNotifier, SettingsState>(
  SettingsNotifier.new,
);