import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';
import 'package:path/path.dart' as p;
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/core/location/gps_accuracy_mode.dart';
import 'package:turbo/core/util/distance_formatter.dart';
import 'package:turbo/features/saved_paths/api.dart';

/// Range bounds for the advanced settings, surfaced so the UI sliders and
/// the persistence layer share the same source of truth.
const int kMinDownloadConcurrency = 1;
const int kMaxDownloadConcurrency = 16;
const int kMinMarkerCacheTtlSeconds = 5;
const int kMaxMarkerCacheTtlSeconds = 300;

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

  /// Hex string for heading arrow color (null = theme primary default).
  final String? markerArrowColorHex;

  /// Hex string for icon outline/border color (null = white default).
  final String? markerOutlineColorHex;

  /// Unit used when rendering distances in the UI.
  final DistanceUnit distanceUnit;

  /// Cap on concurrent tile-region downloads. Read by `DownloadOrchestrator`.
  final int maxConcurrentDownloads;

  /// How long the viewport marker cache holds a query result before refetch.
  final int markerCacheTtlSeconds;

  /// Last-used path-customization style. Pre-seeds the save sheet so the
  /// user's previous choices don't reset on every new path.
  final String? lastPathColorHex;
  final String? lastPathIconKey;
  final bool? lastPathSmoothing;
  final String? lastPathLineStyleKey;

  /// Keep the screen awake while a recording is active.
  final bool keepScreenOnWhileRecording;

  /// Tradeoff between GPS accuracy and battery use during recording.
  final GpsAccuracyMode gpsAccuracyMode;

  /// Set once the user has answered the "enable background recording" prompt
  /// at least once. We respect that choice and don't ask again on every
  /// recording start — they can re-enable background recording from settings
  /// or by granting "Always" location in the OS.
  final bool backgroundLocationPromptSeen;

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
    this.markerArrowColorHex,
    this.markerOutlineColorHex,
    this.distanceUnit = DistanceUnit.metric,
    this.maxConcurrentDownloads = 8,
    this.markerCacheTtlSeconds = 30,
    this.lastPathColorHex,
    this.lastPathIconKey,
    this.lastPathSmoothing,
    this.lastPathLineStyleKey,
    this.keepScreenOnWhileRecording = true,
    this.gpsAccuracyMode = GpsAccuracyMode.high,
    this.backgroundLocationPromptSeen = false,
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
    String? Function()? markerArrowColorHex,
    String? Function()? markerOutlineColorHex,
    DistanceUnit? distanceUnit,
    int? maxConcurrentDownloads,
    int? markerCacheTtlSeconds,
    String? Function()? lastPathColorHex,
    String? Function()? lastPathIconKey,
    bool? Function()? lastPathSmoothing,
    String? Function()? lastPathLineStyleKey,
    bool? keepScreenOnWhileRecording,
    GpsAccuracyMode? gpsAccuracyMode,
    bool? backgroundLocationPromptSeen,
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
      markerArrowColorHex: markerArrowColorHex != null ? markerArrowColorHex() : this.markerArrowColorHex,
      markerOutlineColorHex: markerOutlineColorHex != null ? markerOutlineColorHex() : this.markerOutlineColorHex,
      distanceUnit: distanceUnit ?? this.distanceUnit,
      maxConcurrentDownloads: maxConcurrentDownloads ?? this.maxConcurrentDownloads,
      markerCacheTtlSeconds: markerCacheTtlSeconds ?? this.markerCacheTtlSeconds,
      lastPathColorHex: lastPathColorHex != null ? lastPathColorHex() : this.lastPathColorHex,
      lastPathIconKey: lastPathIconKey != null ? lastPathIconKey() : this.lastPathIconKey,
      lastPathSmoothing: lastPathSmoothing != null ? lastPathSmoothing() : this.lastPathSmoothing,
      lastPathLineStyleKey: lastPathLineStyleKey != null ? lastPathLineStyleKey() : this.lastPathLineStyleKey,
      keepScreenOnWhileRecording: keepScreenOnWhileRecording ?? this.keepScreenOnWhileRecording,
      gpsAccuracyMode: gpsAccuracyMode ?? this.gpsAccuracyMode,
      backgroundLocationPromptSeen:
          backgroundLocationPromptSeen ?? this.backgroundLocationPromptSeen,
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
  static const _markerArrowColorKey = 'markerArrowColor';
  static const _markerOutlineColorKey = 'markerOutlineColor';
  static const _distanceUnitKey = 'distanceUnit';
  static const _maxConcurrentDownloadsKey = 'maxConcurrentDownloads';
  static const _markerCacheTtlSecondsKey = 'markerCacheTtlSeconds';
  static const _lastPathColorKey = 'lastPathColor';
  static const _lastPathIconKey = 'lastPathIcon';
  static const _lastPathSmoothingKey = 'lastPathSmoothing';
  static const _lastPathLineStyleKey = 'lastPathLineStyle';
  static const _keepScreenOnWhileRecordingKey = 'keepScreenOnWhileRecording';
  static const _gpsAccuracyModeKey = 'gpsAccuracyMode';
  static const _backgroundLocationPromptSeenKey =
      'backgroundLocationPromptSeen';

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
    final markerArrowColorHex = prefs.getString(_markerArrowColorKey);
    final markerOutlineColorHex = prefs.getString(_markerOutlineColorKey);

    // Load Advanced settings
    final distanceUnit = DistanceUnit.fromName(prefs.getString(_distanceUnitKey));
    final maxConcurrentDownloads = (prefs.getInt(_maxConcurrentDownloadsKey) ?? 8)
        .clamp(kMinDownloadConcurrency, kMaxDownloadConcurrency);
    final markerCacheTtlSeconds = (prefs.getInt(_markerCacheTtlSecondsKey) ?? 30)
        .clamp(kMinMarkerCacheTtlSeconds, kMaxMarkerCacheTtlSeconds);

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
      markerArrowColorHex: markerArrowColorHex,
      markerOutlineColorHex: markerOutlineColorHex,
      distanceUnit: distanceUnit,
      maxConcurrentDownloads: maxConcurrentDownloads,
      markerCacheTtlSeconds: markerCacheTtlSeconds,
      lastPathColorHex: prefs.getString(_lastPathColorKey),
      lastPathIconKey: prefs.getString(_lastPathIconKey),
      lastPathSmoothing: prefs.getBool(_lastPathSmoothingKey),
      lastPathLineStyleKey: prefs.getString(_lastPathLineStyleKey),
      keepScreenOnWhileRecording:
          prefs.getBool(_keepScreenOnWhileRecordingKey) ?? true,
      gpsAccuracyMode:
          GpsAccuracyMode.fromName(prefs.getString(_gpsAccuracyModeKey)),
      backgroundLocationPromptSeen:
          prefs.getBool(_backgroundLocationPromptSeenKey) ?? false,
    );
  }

  /// Toggles the keep-screen-on-while-recording preference.
  Future<void> setKeepScreenOnWhileRecording(bool value) async {
    if (state.value == null) return;
    state = AsyncData(state.value!.copyWith(keepScreenOnWhileRecording: value));
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool(_keepScreenOnWhileRecordingKey, value);
  }

  /// Updates the GPS accuracy mode used for live recording.
  Future<void> setGpsAccuracyMode(GpsAccuracyMode mode) async {
    if (state.value == null) return;
    state = AsyncData(state.value!.copyWith(gpsAccuracyMode: mode));
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_gpsAccuracyModeKey, mode.name);
  }

  /// Records whether the user has been prompted about background recording.
  /// Once true, the recording start flow won't keep showing the upgrade
  /// dialog on every session start.
  Future<void> setBackgroundLocationPromptSeen(bool seen) async {
    if (state.value == null) return;
    state = AsyncData(
        state.value!.copyWith(backgroundLocationPromptSeen: seen));
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool(_backgroundLocationPromptSeenKey, seen);
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

  /// Sets the heading arrow color. Pass null to revert to theme default.
  Future<void> setMarkerArrowColor(Color? color) async {
    if (state.value == null) return;
    final hex = color != null ? colorToHex(color) : null;
    state = AsyncData(state.value!.copyWith(markerArrowColorHex: () => hex));
    final prefs = await SharedPreferences.getInstance();
    if (hex != null) {
      await prefs.setString(_markerArrowColorKey, hex);
    } else {
      await prefs.remove(_markerArrowColorKey);
    }
  }

  /// Sets the icon outline/border color. Pass null to revert to white default.
  Future<void> setMarkerOutlineColor(Color? color) async {
    if (state.value == null) return;
    final hex = color != null ? colorToHex(color) : null;
    state = AsyncData(state.value!.copyWith(markerOutlineColorHex: () => hex));
    final prefs = await SharedPreferences.getInstance();
    if (hex != null) {
      await prefs.setString(_markerOutlineColorKey, hex);
    } else {
      await prefs.remove(_markerOutlineColorKey);
    }
  }

  /// Persists the user's distance unit choice.
  Future<void> setDistanceUnit(DistanceUnit unit) async {
    if (state.value == null) return;
    state = AsyncData(state.value!.copyWith(distanceUnit: unit));
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_distanceUnitKey, unit.name);
  }

  /// Persists the max concurrent download cap, clamped to the supported range.
  Future<void> setMaxConcurrentDownloads(int value) async {
    if (state.value == null) return;
    final clamped =
        value.clamp(kMinDownloadConcurrency, kMaxDownloadConcurrency);
    state = AsyncData(state.value!.copyWith(maxConcurrentDownloads: clamped));
    final prefs = await SharedPreferences.getInstance();
    await prefs.setInt(_maxConcurrentDownloadsKey, clamped);
  }

  /// Persists the marker viewport cache TTL in seconds.
  Future<void> setMarkerCacheTtlSeconds(int value) async {
    if (state.value == null) return;
    final clamped =
        value.clamp(kMinMarkerCacheTtlSeconds, kMaxMarkerCacheTtlSeconds);
    state = AsyncData(state.value!.copyWith(markerCacheTtlSeconds: clamped));
    final prefs = await SharedPreferences.getInstance();
    await prefs.setInt(_markerCacheTtlSecondsKey, clamped);
  }

  /// Records the path style the user just chose so the next save sheet
  /// can pre-fill the same values. Each argument is overwritten on every
  /// call — passing null clears the matching slot rather than leaving the
  /// previous value in place.
  Future<void> setLastPathStyle({
    String? colorHex,
    String? iconKey,
    bool? smoothing,
    String? lineStyleKey,
  }) async {
    if (state.value == null) return;
    state = AsyncData(state.value!.copyWith(
      lastPathColorHex: () => colorHex,
      lastPathIconKey: () => iconKey,
      lastPathSmoothing: () => smoothing,
      lastPathLineStyleKey: () => lineStyleKey,
    ));
    final prefs = await SharedPreferences.getInstance();
    if (colorHex == null) {
      await prefs.remove(_lastPathColorKey);
    } else {
      await prefs.setString(_lastPathColorKey, colorHex);
    }
    if (iconKey == null) {
      await prefs.remove(_lastPathIconKey);
    } else {
      await prefs.setString(_lastPathIconKey, iconKey);
    }
    if (smoothing == null) {
      await prefs.remove(_lastPathSmoothingKey);
    } else {
      await prefs.setBool(_lastPathSmoothingKey, smoothing);
    }
    if (lineStyleKey == null) {
      await prefs.remove(_lastPathLineStyleKey);
    } else {
      await prefs.setString(_lastPathLineStyleKey, lineStyleKey);
    }
  }
}

/// The provider for accessing the SettingsNotifier and its state.
///
/// This is the single source of truth for all user settings.
final settingsProvider = AsyncNotifierProvider<SettingsNotifier, SettingsState>(
  SettingsNotifier.new,
);