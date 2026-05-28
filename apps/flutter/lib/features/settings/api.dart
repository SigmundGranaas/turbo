/// The public API for the Settings feature.
library;

export 'package:turbo/core/util/distance_formatter.dart'
    show DistanceUnit, formatDistance;
export 'package:turbo/core/location/gps_accuracy_mode.dart'
    show GpsAccuracyMode;
export 'data/settings_provider.dart' show settingsProvider, SettingsState;
export 'widgets/location_icon_picker_sheet.dart' show showLocationIconPickerSheet;
export 'widgets/settings_page.dart';
export 'widgets/sections/about_settings_page.dart' show AboutSettingsPage;
export 'widgets/sections/notifications_settings_page.dart'
    show NotificationsSettingsPage;
