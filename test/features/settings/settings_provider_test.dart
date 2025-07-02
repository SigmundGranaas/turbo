import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
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
  });
}