import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/app/l10n/app_localizations.dart';

import '../api.dart';

/// Modal checklist sheet for the per-marker weather metrics.
///
/// Opens from the [WeatherSection] header. Edits are buffered in local state
/// until the user taps Save; tapping Cancel (or dismissing the sheet) discards
/// changes — keeping toggle-then-back-out fast for accidental opens.
class WeatherMetricsSheet extends ConsumerStatefulWidget {
  final String markerUuid;
  const WeatherMetricsSheet({super.key, required this.markerUuid});

  @override
  ConsumerState<WeatherMetricsSheet> createState() =>
      _WeatherMetricsSheetState();
}

class _WeatherMetricsSheetState extends ConsumerState<WeatherMetricsSheet> {
  late Set<WeatherMetric> _selected;
  bool _seeded = false;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final prefs = ref.watch(markerWeatherPrefsProvider(widget.markerUuid));
    if (!_seeded) {
      _selected = {...prefs.metrics};
      _seeded = true;
    }

    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;
    final atm = WeatherMetric.values
        .where((m) => m.source == WeatherMetricSource.atmospheric)
        .toList();
    final marine = WeatherMetric.values
        .where((m) => m.source == WeatherMetricSource.marine)
        .toList();

    return SafeArea(
      child: SingleChildScrollView(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 16, 16, 16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  Expanded(
                    child: Text(
                      l10n.weatherCustomize,
                      style: textTheme.titleLarge,
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.close),
                    tooltip: l10n.cancel,
                    onPressed: () => Navigator.pop(context),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              _SectionHeader(label: l10n.weatherAtmospheric),
              for (final m in atm)
                CheckboxListTile(
                  contentPadding: EdgeInsets.zero,
                  title: Text(_label(l10n, m)),
                  value: _selected.contains(m),
                  onChanged: (v) => setState(() {
                    if (v == true) {
                      _selected.add(m);
                    } else {
                      _selected.remove(m);
                    }
                  }),
                ),
              const SizedBox(height: 12),
              _SectionHeader(label: l10n.weatherMarine),
              for (final m in marine)
                CheckboxListTile(
                  contentPadding: EdgeInsets.zero,
                  title: Text(_label(l10n, m)),
                  value: _selected.contains(m),
                  onChanged: (v) => setState(() {
                    if (v == true) {
                      _selected.add(m);
                    } else {
                      _selected.remove(m);
                    }
                  }),
                ),
              const SizedBox(height: 16),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  TextButton(
                    onPressed: () => Navigator.pop(context),
                    child: Text(l10n.cancel),
                  ),
                  const SizedBox(width: 8),
                  FilledButton(
                    style: FilledButton.styleFrom(
                      backgroundColor: colorScheme.primary,
                      foregroundColor: colorScheme.onPrimary,
                    ),
                    onPressed: _save,
                    child: Text(l10n.save),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _save() async {
    final navigator = Navigator.of(context);
    await ref
        .read(markerWeatherPrefsProvider(widget.markerUuid).notifier)
        .setMetrics(_selected);
    if (navigator.canPop()) {
      navigator.pop();
    }
  }
}

class _SectionHeader extends StatelessWidget {
  final String label;
  const _SectionHeader({required this.label});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Text(
        label,
        style: textTheme.labelLarge?.copyWith(
          color: colorScheme.primary,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}

String _label(AppLocalizations l10n, WeatherMetric metric) {
  switch (metric) {
    case WeatherMetric.temperature:
      return l10n.metricTemperature;
    case WeatherMetric.precipitation:
      return l10n.metricPrecipitation;
    case WeatherMetric.snow:
      return l10n.metricSnow;
    case WeatherMetric.wind:
      return l10n.metricWind;
    case WeatherMetric.humidity:
      return l10n.metricHumidity;
    case WeatherMetric.pressure:
      return l10n.metricPressure;
    case WeatherMetric.cloudCover:
      return l10n.metricCloudCover;
    case WeatherMetric.uvIndex:
      return l10n.metricUv;
    case WeatherMetric.waveHeight:
      return l10n.metricWaveHeight;
    case WeatherMetric.waveDirection:
      return l10n.metricWaveDirection;
    case WeatherMetric.waterTemperature:
      return l10n.metricWaterTemperature;
  }
}
