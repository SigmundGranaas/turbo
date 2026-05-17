import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/core/util/distance_formatter.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  group('SettingsNotifier', () {
    // Helper to create a ProviderContainer with mocked SharedPreferences
    ProviderContainer createContainer(Map<String, Object> initialPrefs) {
      SharedPreferences.setMockInitialValues(initialPrefs);
      return ProviderContainer();
    }

    test('initial state loads with default values when SharedPreferences is empty', () async {
      // Arrange
      final container = createContainer({});

      // Act
      // Wait for the async initialization to complete
      final initialState = await container.read(settingsProvider.future);

      // Assert
      expect(initialState.themeMode, ThemeMode.system);
      expect(initialState.locale, const Locale('en'));
    });

    test('initial state loads with saved values from SharedPreferences', () async {
      // Arrange
      final container = createContainer({
        'themeMode': 'dark',
        'locale': 'nb',
      });

      // Act
      final loadedState = await container.read(settingsProvider.future);

      // Assert
      expect(loadedState.themeMode, ThemeMode.dark);
      expect(loadedState.locale, const Locale('nb'));
    });

    test('setThemeMode updates the state and persists to SharedPreferences', () async {
      // Arrange
      final container = createContainer({});
      await container.read(settingsProvider.future); // Ensure initialized

      // Act
      await container.read(settingsProvider.notifier).setThemeMode(ThemeMode.light);

      // Assert - State
      final updatedState = container.read(settingsProvider).value;
      expect(updatedState?.themeMode, ThemeMode.light);

      // Assert - Persistence
      final prefs = await SharedPreferences.getInstance();
      expect(prefs.getString('themeMode'), 'light');
    });

    test('setLocale updates the state and persists to SharedPreferences', () async {
      // Arrange
      final container = createContainer({});
      await container.read(settingsProvider.future); // Ensure initialized

      // Act
      await container.read(settingsProvider.notifier).setLocale(const Locale('nb'));

      // Assert - State
      final updatedState = container.read(settingsProvider).value;
      expect(updatedState?.locale, const Locale('nb'));

      // Assert - Persistence
      final prefs = await SharedPreferences.getInstance();
      expect(prefs.getString('locale'), 'nb');
    });

    test('setting theme to system removes the key from SharedPreferences', () async {
      // Arrange
      final container = createContainer({'themeMode': 'dark'});
      await container.read(settingsProvider.future); // Ensure initialized

      // Act
      await container.read(settingsProvider.notifier).setThemeMode(ThemeMode.system);

      // Assert - State
      final updatedState = container.read(settingsProvider).value;
      expect(updatedState?.themeMode, ThemeMode.system);

      // Assert - Persistence
      final prefs = await SharedPreferences.getInstance();
      expect(prefs.containsKey('themeMode'), isFalse);
    });

    group('advanced settings', () {
      test('defaults are metric, concurrency=8, marker TTL=30 s when empty',
          () async {
        final container = createContainer({});
        final state = await container.read(settingsProvider.future);
        expect(state.distanceUnit, DistanceUnit.metric);
        expect(state.maxConcurrentDownloads, 8);
        expect(state.markerCacheTtlSeconds, 30);
      });

      test('loaded values are clamped to the supported range', () async {
        final container = createContainer({
          'maxConcurrentDownloads': 999,
          'markerCacheTtlSeconds': -50,
        });
        final state = await container.read(settingsProvider.future);
        expect(state.maxConcurrentDownloads, kMaxDownloadConcurrency);
        expect(state.markerCacheTtlSeconds, kMinMarkerCacheTtlSeconds);
      });

      test('setDistanceUnit updates state and persists by name', () async {
        final container = createContainer({});
        await container.read(settingsProvider.future);

        await container
            .read(settingsProvider.notifier)
            .setDistanceUnit(DistanceUnit.imperial);

        expect(container.read(settingsProvider).value?.distanceUnit,
            DistanceUnit.imperial);
        final prefs = await SharedPreferences.getInstance();
        expect(prefs.getString('distanceUnit'), 'imperial');
      });

      test('setMaxConcurrentDownloads clamps and persists', () async {
        final container = createContainer({});
        await container.read(settingsProvider.future);
        final notifier = container.read(settingsProvider.notifier);

        await notifier.setMaxConcurrentDownloads(99);
        expect(container.read(settingsProvider).value?.maxConcurrentDownloads,
            kMaxDownloadConcurrency);

        await notifier.setMaxConcurrentDownloads(-3);
        expect(container.read(settingsProvider).value?.maxConcurrentDownloads,
            kMinDownloadConcurrency);

        final prefs = await SharedPreferences.getInstance();
        expect(prefs.getInt('maxConcurrentDownloads'),
            kMinDownloadConcurrency);
      });

      test('setMarkerCacheTtlSeconds clamps and persists', () async {
        final container = createContainer({});
        await container.read(settingsProvider.future);

        await container
            .read(settingsProvider.notifier)
            .setMarkerCacheTtlSeconds(2);

        expect(container.read(settingsProvider).value?.markerCacheTtlSeconds,
            kMinMarkerCacheTtlSeconds);
        final prefs = await SharedPreferences.getInstance();
        expect(prefs.getInt('markerCacheTtlSeconds'),
            kMinMarkerCacheTtlSeconds);
      });
    });
  });
}