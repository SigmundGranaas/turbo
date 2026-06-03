import 'package:flutter/material.dart';

import 'package:turbo/features/avalanche_forecast/api.dart'
    show showAvalancheWarningSheet;
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/markers/api.dart' show Marker;
import 'package:turbo/features/weather/api.dart' show showWeatherDetailSheet;

/// The conditions sources this build ships, composed at the app shell (which
/// is allowed to import the forecast features). Each wraps an existing sheet,
/// so the new seam reuses the current weather/avalanche UIs rather than
/// reimplementing them. Adding tides/ocean is just another entry here.
MapConditionsRegistry buildDefaultMapConditionsRegistry() {
  return MapConditionsRegistry([
    ConditionsSource(
      id: 'weather',
      label: 'Weather',
      icon: Icons.wb_sunny_outlined,
      show: (context, point) => showWeatherDetailSheet(
        context,
        Marker(title: 'Weather', position: point),
      ),
    ),
    ConditionsSource(
      id: 'avalanche',
      label: 'Avalanche forecast',
      icon: Icons.ac_unit,
      show: (context, point) => showAvalancheWarningSheet(context, point),
    ),
  ]);
}
