import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../models/activity_analysis.dart';
import '../../models/driver_keys.dart';
import 'activity_weather_panel.dart';

/// One driver to pluck off [ActivityAnalysis.drivers] and render as a
/// metric chip in the weather panel header. The key must come from
/// [DriverKeys] — the contract test enforces it.
class WeatherDriverConfig {
  final String driverKey;
  final IconData icon;

  /// How to format the raw double value into a chip label, e.g.
  /// `(v) => '${v.toStringAsFixed(0)} m/s'`.
  final String Function(double value) format;

  const WeatherDriverConfig(this.driverKey, this.icon, this.format);
}

/// Convenience constructors for the most common metric shapes. Cuts
/// the per-sheet boilerplate to one line per metric.
class _Formatters {
  static String windMs(double v) => '${v.toStringAsFixed(0)} m/s';
  static String mm24h(double v) => '${v.toStringAsFixed(0)} mm 24h';
  static String snowCm24h(double v) => '${v.toStringAsFixed(0)} cm 24h';
  static String snowDepthCm(double v) => '${v.toStringAsFixed(0)} cm base';
  static String waterC(double v) => '${v.toStringAsFixed(0)}° water';
  static String waveM(double v) => '${v.toStringAsFixed(1)} m swell';
  static String vizM(double v) => '${v.toStringAsFixed(0)} m viz';
  static String flow(double v) => '${v.toStringAsFixed(1)} m³/s flow';
  static String pressureMb(double v) =>
      '${v >= 0 ? '+' : ''}${v.toStringAsFixed(1)} mb/24h';
}

/// Preset metric configs keyed off [DriverKeys]. Sheets pick a handful
/// of these instead of recreating icon/format mappings six times.
class WeatherMetrics {
  static const wind = WeatherDriverConfig(
      DriverKeys.wind, Icons.air, _Formatters.windMs);
  static const rain24h = WeatherDriverConfig(
      DriverKeys.rain24h, Icons.umbrella, _Formatters.mm24h);
  static const freshSnow24h = WeatherDriverConfig(
      DriverKeys.freshSnow24h, Icons.ac_unit, _Formatters.snowCm24h);
  static const snowDepth = WeatherDriverConfig(
      DriverKeys.snowDepth, Icons.landscape, _Formatters.snowDepthCm);
  static const seaTemp = WeatherDriverConfig(
      DriverKeys.seaTemp, Icons.water, _Formatters.waterC);
  static const waterTemp = WeatherDriverConfig(
      DriverKeys.waterTemp, Icons.water, _Formatters.waterC);
  static const waveHeight = WeatherDriverConfig(
      DriverKeys.waveHeight, Icons.waves, _Formatters.waveM);
  static const vizMeters = WeatherDriverConfig(
      DriverKeys.vizMeters, Icons.visibility, _Formatters.vizM);
  static const flowCumecs = WeatherDriverConfig(
      DriverKeys.flowCumecs, Icons.water_drop, _Formatters.flow);
  static const pressureTrend = WeatherDriverConfig(
      DriverKeys.pressureTrend, Icons.compress, _Formatters.pressureMb);
}

/// Drop-in weather panel for a per-kind detail sheet. Watches an
/// [AsyncValue] of [ActivityAnalysis] and renders the four states
/// (loading / ready / error / noData) on the underlying
/// [ActivityWeatherPanel] without the caller having to hand-write the
/// `analysisAsync.when(...)` ladder + driver lookup each time.
///
/// This is the shared replacement for the per-kind `_WeatherPanel`
/// helper class that used to live in each detail sheet — saves ~80
/// lines per kind and centralises the driver-key contract.
class ActivityWeatherPanelFromAnalysis extends StatelessWidget {
  final AsyncValue<ActivityAnalysis> analysisAsync;
  final String title;
  final String? subtitle;
  final Color accent;

  /// What to render below the big temperature. Kind-specific copy
  /// ("XC track surface forecast", "Trail weather forecast", …).
  final String summaryBlurb;

  /// Metrics to show in the right-column chip stack. Order matters —
  /// the panel takes the first 4.
  final List<WeatherDriverConfig> metrics;

  final VoidCallback? onRefresh;

  const ActivityWeatherPanelFromAnalysis({
    super.key,
    required this.analysisAsync,
    required this.title,
    required this.accent,
    required this.summaryBlurb,
    required this.metrics,
    this.subtitle,
    this.onRefresh,
  });

  @override
  Widget build(BuildContext context) {
    return analysisAsync.when(
      loading: () => ActivityWeatherPanel(
        title: title,
        accent: accent,
        loadingState: WeatherLoadingState.loading,
        onRefresh: onRefresh,
      ),
      error: (_, _) => ActivityWeatherPanel(
        title: title,
        accent: accent,
        loadingState: WeatherLoadingState.error,
        onRefresh: onRefresh,
      ),
      data: (a) {
        final summary = _summarise(a);
        return ActivityWeatherPanel(
          title: title,
          subtitle: subtitle ?? 'Met.no',
          accent: accent,
          loadingState: summary == null
              ? WeatherLoadingState.noData
              : WeatherLoadingState.ready,
          summary: summary,
          onRefresh: onRefresh,
        );
      },
    );
  }

  WeatherSummary? _summarise(ActivityAnalysis a) {
    final tempBand = _driver(a, DriverKeys.tempBand);
    final chips = <WeatherMetric>[];
    for (final cfg in metrics) {
      final v = _driver(a, cfg.driverKey);
      if (v == null) continue;
      chips.add(WeatherMetric(cfg.icon, cfg.format(v)));
    }
    if (tempBand == null && chips.isEmpty) return null;
    return WeatherSummary(
      symbol: Icons.cloud_outlined,
      temperature: tempBand == null ? '—' : '${tempBand.toStringAsFixed(0)}°',
      summary: summaryBlurb,
      metrics: chips,
    );
  }

  static double? _driver(ActivityAnalysis a, String key) {
    for (final d in a.drivers) {
      if (d.key == key) return d.value;
    }
    return null;
  }
}
