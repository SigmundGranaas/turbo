import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';
import 'package:path/path.dart' as p;
import 'package:shared_preferences/shared_preferences.dart';

/// Defines the state for all user-configurable settings.
@immutable
class SettingsState {
  final ThemeMode themeMode;
  final Locale locale;
  final double drawSensitivity;
  final bool smoothLine;
  final bool showIntermediatePoints;

  /// One of 'default', 'builtin', 'custom'.
  final String locationIconType;

  /// For builtin icons, the icon key (e.g. 'Fjell').
  final String? locationIconKey;

  /// Relative path to custom image in app docs dir.
  final String? locationImagePath;

  /// Scale multiplier for the position marker (0.5–2.0).
  final double locationMarkerSize;

  /// Whether to show a heading direction arrow behind the marker.
  final bool showHeadingArrow;

  const SettingsState({
    required this.themeMode,
    required this.locale,
    required this.drawSensitivity,
    required this.smoothLine,
    required this.showIntermediatePoints,
    this.locationIconType = 'default',
    this.locationIconKey,
    this.locationImagePath,
    this.locationMarkerSize = 1.0,
    this.showHeadingArrow = false,
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
    String? locationIconType,
    String? Function()? locationIconKey,
    String? Function()? locationImagePath,
    double? locationMarkerSize,
    bool? showHeadingArrow,
  }) {
    return SettingsState(
      themeMode: themeMode ?? this.themeMode,
      locale: locale ?? this.locale,
      drawSensitivity: drawSensitivity ?? this.drawSensitivity,
      smoothLine: smoothLine ?? this.smoothLine,
      showIntermediatePoints: showIntermediatePoints ?? this.showIntermediatePoints,
      locationIconType: locationIconType ?? this.locationIconType,
      locationIconKey: locationIconKey != null ? locationIconKey() : this.locationIconKey,
      locationImagePath: locationImagePath != null ? locationImagePath() : this.locationImagePath,
      locationMarkerSize: locationMarkerSize ?? this.locationMarkerSize,
      showHeadingArrow: showHeadingArrow ?? this.showHeadingArrow,
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
  static const _locationIconTypeKey = 'locationIconType';
  static const _locationIconKeyKey = 'locationIconKey';
  static const _locationImagePathKey = 'locationImagePath';
  static const _locationMarkerSizeKey = 'locationMarkerSize';
  static const _showHeadingArrowKey = 'showHeadingArrow';

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

    // Load Location Marker Settings
    final locationIconType = prefs.getString(_locationIconTypeKey) ?? 'default';
    final locationIconKey = prefs.getString(_locationIconKeyKey);
    final locationImagePath = prefs.getString(_locationImagePathKey);
    final locationMarkerSize = prefs.getDouble(_locationMarkerSizeKey) ?? 1.0;
    final showHeadingArrow = prefs.getBool(_showHeadingArrowKey) ?? false;

    return SettingsState(
      themeMode: themeMode,
      locale: locale,
      drawSensitivity: drawSensitivity,
      smoothLine: smoothLine,
      showIntermediatePoints: showIntermediatePoints,
      locationIconType: locationIconType,
      locationIconKey: locationIconKey,
      locationImagePath: locationImagePath,
      locationMarkerSize: locationMarkerSize,
      showHeadingArrow: showHeadingArrow,
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

  /// Sets the location icon to a builtin icon by key.
  Future<void> setLocationBuiltinIcon(String iconKey) async {
    if (state.value == null) return;
    state = AsyncData(state.value!.copyWith(
      locationIconType: 'builtin',
      locationIconKey: () => iconKey,
      locationImagePath: () => null,
    ));
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_locationIconTypeKey, 'builtin');
    await prefs.setString(_locationIconKeyKey, iconKey);
    await prefs.remove(_locationImagePathKey);
  }

  /// Sets the location icon to a custom image. Copies the file to app docs dir.
  Future<void> setLocationImage(String sourcePath) async {
    if (state.value == null) return;
    final docsDir = await getApplicationDocumentsDirectory();
    final iconDir = Directory(p.join(docsDir.path, 'location_icons'));
    if (!iconDir.existsSync()) {
      iconDir.createSync(recursive: true);
    }

    final ext = p.extension(sourcePath);
    final destFileName = 'location_icon$ext';
    final destPath = p.join(iconDir.path, destFileName);
    await File(sourcePath).copy(destPath);

    final relativePath = p.join('location_icons', destFileName);
    state = AsyncData(state.value!.copyWith(
      locationIconType: 'custom',
      locationIconKey: () => null,
      locationImagePath: () => relativePath,
    ));
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_locationIconTypeKey, 'custom');
    await prefs.remove(_locationIconKeyKey);
    await prefs.setString(_locationImagePathKey, relativePath);
  }

  /// Resets the location icon to the default blue dot.
  Future<void> resetLocationIcon() async {
    if (state.value == null) return;
    state = AsyncData(state.value!.copyWith(
      locationIconType: 'default',
      locationIconKey: () => null,
      locationImagePath: () => null,
    ));
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_locationIconTypeKey, 'default');
    await prefs.remove(_locationIconKeyKey);
    await prefs.remove(_locationImagePathKey);
  }

  /// Updates the location marker size multiplier.
  Future<void> setLocationMarkerSize(double value) async {
    if (state.value == null) return;
    state = AsyncData(state.value!.copyWith(locationMarkerSize: value));
    final prefs = await SharedPreferences.getInstance();
    await prefs.setDouble(_locationMarkerSizeKey, value);
  }

  /// Toggles the heading arrow display.
  Future<void> setShowHeadingArrow(bool value) async {
    if (state.value == null) return;
    state = AsyncData(state.value!.copyWith(showHeadingArrow: value));
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool(_showHeadingArrowKey, value);
  }
}

/// The provider for accessing the SettingsNotifier and its state.
///
/// This is the single source of truth for all user settings.
final settingsProvider = AsyncNotifierProvider<SettingsNotifier, SettingsState>(
  SettingsNotifier.new,
);