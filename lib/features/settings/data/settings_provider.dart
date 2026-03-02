import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Defines the state for all user-configurable settings.
@immutable
class SettingsState {
  final ThemeMode themeMode;
  final Locale locale;
  final double drawSensitivity;
  final bool smoothLine;
  final bool showIntermediatePoints;

  const SettingsState({
    required this.themeMode,
    required this.locale,
    required this.drawSensitivity,
    required this.smoothLine,
    required this.showIntermediatePoints,
  });

  // Default initial state
  factory SettingsState.initial() => const SettingsState(
    themeMode: ThemeMode.system,
    locale: Locale('en'),
    drawSensitivity: 15.0,
    smoothLine: true,
    showIntermediatePoints: false,
  );

  SettingsState copyWith({
    ThemeMode? themeMode,
    Locale? locale,
    double? drawSensitivity,
    bool? smoothLine,
    bool? showIntermediatePoints,
  }) {
    return SettingsState(
      themeMode: themeMode ?? this.themeMode,
      locale: locale ?? this.locale,
      drawSensitivity: drawSensitivity ?? this.drawSensitivity,
      smoothLine: smoothLine ?? this.smoothLine,
      showIntermediatePoints: showIntermediatePoints ?? this.showIntermediatePoints,
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
  static const _drawSensitivityKey = 'drawSensitivity';
  static const _smoothLineKey = 'smoothLine';
  static const _showIntermediatePointsKey = 'showIntermediatePoints';

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

    // Load Draw Sensitivity
    final drawSensitivity = prefs.getDouble(_drawSensitivityKey) ?? 15.0;

    // Load Drawing Preferences
    final smoothLine = prefs.getBool(_smoothLineKey) ?? true;
    final showIntermediatePoints = prefs.getBool(_showIntermediatePointsKey) ?? false;

    return SettingsState(
      themeMode: themeMode,
      locale: locale,
      drawSensitivity: drawSensitivity,
      smoothLine: smoothLine,
      showIntermediatePoints: showIntermediatePoints,
    );
  }

  /// Updates the draw sensitivity and persists it.
  Future<void> setDrawSensitivity(double value) async {
    if (state.value == null) return;
    state = AsyncData(state.value!.copyWith(drawSensitivity: value));
    final prefs = await SharedPreferences.getInstance();
    await prefs.setDouble(_drawSensitivityKey, value);
  }

  /// Updates the smooth line setting and persists it.
  Future<void> setSmoothLine(bool value) async {
    if (state.value == null) return;
    state = AsyncData(state.value!.copyWith(smoothLine: value));
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool(_smoothLineKey, value);
  }

  /// Updates the show intermediate points setting and persists it.
  Future<void> setShowIntermediatePoints(bool value) async {
    if (state.value == null) return;
    state = AsyncData(state.value!.copyWith(showIntermediatePoints: value));
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool(_showIntermediatePointsKey, value);
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