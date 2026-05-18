import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/avalanche_forecast/api.dart'
    show AvalancheWarningBadge;
import 'package:turbo/features/markers/api.dart' show Marker;

import '../api.dart';
import 'weather_widgets_internal.dart';

/// Opens the weather detail as an expanding bottom sheet anchored over the
/// current route. Returns once the sheet is dismissed.
Future<void> showWeatherDetailSheet(BuildContext context, Marker marker) {
  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    showDragHandle: false,
    useSafeArea: true,
    backgroundColor: Colors.transparent,
    builder: (_) => WeatherDetailSheet(marker: marker),
  );
}

/// Bottom sheet showing the full forecast for [marker].
///
/// Layout:
///   • Day strip (horizontal) — tap a day to load its hours.
///   • Preset tabs (Hourly / Wind / Precipitation / Sea*) — swipeable.
///   • Hour list for `(selectedDay, selectedPreset)`.
///
/// (* Sea preset is only present when MET has marine data for the coord.)
class WeatherDetailSheet extends ConsumerStatefulWidget {
  final Marker marker;
  const WeatherDetailSheet({super.key, required this.marker});

  @override
  ConsumerState<WeatherDetailSheet> createState() =>
      _WeatherDetailSheetState();
}

enum _Preset { hourly, wind, precipitation, sea }

class _WeatherDetailSheetState extends ConsumerState<WeatherDetailSheet>
    with SingleTickerProviderStateMixin {
  int _dayIndex = 0;
  int _presetIndex = 0;
  late final PageController _presetController;

  @override
  void initState() {
    super.initState();
    _presetController = PageController();
  }

  @override
  void dispose() {
    _presetController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final forecast =
        ref.watch(weatherForecastProvider(widget.marker.position));

    return DraggableScrollableSheet(
      initialChildSize: 0.85,
      minChildSize: 0.5,
      maxChildSize: 0.95,
      expand: false,
      builder: (context, scrollController) {
        return Container(
          decoration: BoxDecoration(
            color: colorScheme.surface,
            borderRadius:
                const BorderRadius.vertical(top: Radius.circular(24)),
          ),
          child: forecast.when(
            loading: () => _loadingShell(),
            error: (_, _) => _errorShell(),
            data: (f) => _dataShell(f, scrollController),
          ),
        );
      },
    );
  }

  Widget _loadingShell() => Column(
        children: const [
          _DragHandle(),
          SizedBox(height: 200),
          Center(child: CircularProgressIndicator()),
        ],
      );

  Widget _errorShell() {
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        const _DragHandle(),
        const SizedBox(height: 60),
        Icon(Icons.cloud_off_outlined, size: 48, color: colorScheme.error),
        const SizedBox(height: 12),
        Text(l10n.weatherLoadError),
        const SizedBox(height: 12),
        FilledButton(
          onPressed: () => ref
              .read(weatherForecastProvider(widget.marker.position).notifier)
              .refresh(),
          child: Text(l10n.weatherRetry),
        ),
      ],
    );
  }

  Widget _dataShell(WeatherForecast f, ScrollController scrollController) {
    final daily = f.dailySummaries().take(9).toList();
    if (daily.isEmpty) return _errorShell();
    final clampedDay = _dayIndex.clamp(0, daily.length - 1);
    final presets = _availablePresets(f);
    final clampedPreset = _presetIndex.clamp(0, presets.length - 1);

    final selectedDay = daily[clampedDay].date;
    final sunForDay = f.sun[DateTime(
      selectedDay.year,
      selectedDay.month,
      selectedDay.day,
    )];
    final moonForDay = f.moon[DateTime(
      selectedDay.year,
      selectedDay.month,
      selectedDay.day,
    )];

    return Column(
      children: [
        const _DragHandle(),
        _Header(title: widget.marker.title),
        if (f.hasActiveAlerts)
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 4, 16, 4),
            child: MetAlertBanner(alert: f.topAlert!),
          ),
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 4, 16, 4),
          child: AvalancheWarningBadge(position: widget.marker.position),
        ),
        if (sunForDay != null)
          _SunRow(sun: sunForDay, moon: moonForDay),
        const SizedBox(height: 4),
        _DayStrip(
          days: daily,
          selectedIndex: clampedDay,
          onSelect: (i) => setState(() => _dayIndex = i),
        ),
        const SizedBox(height: 8),
        _PresetTabs(
          presets: presets,
          selectedIndex: clampedPreset,
          onSelect: (i) {
            setState(() => _presetIndex = i);
            _presetController.animateToPage(
              i,
              duration: const Duration(milliseconds: 220),
              curve: Curves.easeOut,
            );
          },
        ),
        const SizedBox(height: 4),
        Expanded(
          child: PageView.builder(
            controller: _presetController,
            onPageChanged: (i) => setState(() => _presetIndex = i),
            itemCount: presets.length,
            itemBuilder: (context, i) => _PresetBody(
              preset: presets[i],
              forecast: f,
              day: daily[clampedDay].date,
              scrollController: scrollController,
            ),
          ),
        ),
        const _AttributionFooter(),
      ],
    );
  }

  List<_Preset> _availablePresets(WeatherForecast f) => [
        _Preset.hourly,
        _Preset.wind,
        _Preset.precipitation,
        if (f.hasMarineData) _Preset.sea,
      ];
}

class _DragHandle extends StatelessWidget {
  const _DragHandle();
  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 10, bottom: 8),
      child: Center(
        child: Container(
          width: 40,
          height: 4,
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.onSurfaceVariant.withValues(
                  alpha: 0.4,
                ),
            borderRadius: BorderRadius.circular(2),
          ),
        ),
      ),
    );
  }
}

class _Header extends StatelessWidget {
  final String title;
  const _Header({required this.title});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 4, 12, 4),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: textTheme.titleMedium,
                  overflow: TextOverflow.ellipsis,
                ),
                Text(
                  context.l10n.weatherForecast,
                  style: textTheme.bodySmall
                      ?.copyWith(color: colorScheme.onSurfaceVariant),
                ),
              ],
            ),
          ),
          IconButton(
            icon: const Icon(Icons.close),
            tooltip: MaterialLocalizations.of(context).closeButtonLabel,
            onPressed: () => Navigator.of(context).maybePop(),
          ),
        ],
      ),
    );
  }
}

class _DayStrip extends StatelessWidget {
  final List<DailySummary> days;
  final int selectedIndex;
  final ValueChanged<int> onSelect;
  const _DayStrip({
    required this.days,
    required this.selectedIndex,
    required this.onSelect,
  });

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      key: const Key('weather-detail-day-strip'),
      height: 128,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 16),
        itemCount: days.length,
        separatorBuilder: (_, _) => const SizedBox(width: 8),
        itemBuilder: (context, i) => _DayChip(
          summary: days[i],
          selected: i == selectedIndex,
          onTap: () => onSelect(i),
        ),
      ),
    );
  }
}

class _DayChip extends StatelessWidget {
  final DailySummary summary;
  final bool selected;
  final VoidCallback onTap;
  const _DayChip({
    required this.summary,
    required this.selected,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;
    final bg = selected
        ? colorScheme.primaryContainer
        : colorScheme.surfaceContainerHigh;
    final fg = selected
        ? colorScheme.onPrimaryContainer
        : colorScheme.onSurface;
    final dayLabel = DateFormat('EEE d').format(summary.date);
    return Material(
      color: bg,
      borderRadius: BorderRadius.circular(14),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(14),
        child: Container(
          width: 84,
          padding: const EdgeInsets.symmetric(vertical: 8),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Text(dayLabel,
                  style: textTheme.bodyMedium?.copyWith(color: fg)),
              const SizedBox(height: 4),
              WeatherSymbolIcon(symbol: summary.middaySymbol, size: 28),
              const SizedBox(height: 4),
              Text(
                '${summary.maxTempC.toStringAsFixed(0)}° / ${summary.minTempC.toStringAsFixed(0)}°',
                style: textTheme.bodySmall?.copyWith(color: fg),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _PresetTabs extends StatelessWidget {
  final List<_Preset> presets;
  final int selectedIndex;
  final ValueChanged<int> onSelect;
  const _PresetTabs({
    required this.presets,
    required this.selectedIndex,
    required this.onSelect,
  });

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      key: const Key('weather-detail-preset-tabs'),
      height: 40,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 16),
        itemCount: presets.length,
        separatorBuilder: (_, _) => const SizedBox(width: 8),
        itemBuilder: (context, i) {
          final preset = presets[i];
          return ChoiceChip(
            key: Key('preset-chip-${preset.name}'),
            label: Text(_labelFor(context, preset)),
            selected: i == selectedIndex,
            onSelected: (_) => onSelect(i),
          );
        },
      ),
    );
  }

  String _labelFor(BuildContext context, _Preset preset) {
    final l10n = context.l10n;
    return switch (preset) {
      _Preset.hourly => l10n.weatherPresetHourly,
      _Preset.wind => l10n.weatherWindLabel,
      _Preset.precipitation => l10n.weatherPrecipitationLabel,
      _Preset.sea => l10n.weatherMarineSection,
    };
  }
}

class _PresetBody extends StatelessWidget {
  final _Preset preset;
  final WeatherForecast forecast;
  final DateTime day;
  final ScrollController scrollController;
  const _PresetBody({
    required this.preset,
    required this.forecast,
    required this.day,
    required this.scrollController,
  });

  @override
  Widget build(BuildContext context) {
    final atmHours = forecast.atmospheric
        .where((p) => _sameLocalDay(p.timeUtc, day))
        .toList();
    final marineHours = forecast.marine
        .where((p) => _sameLocalDay(p.timeUtc, day))
        .toList();
    if (preset == _Preset.sea) {
      if (marineHours.isEmpty) {
        return _EmptyPreset(label: context.l10n.weatherMarineEmpty);
      }
      return ListView.separated(
        controller: scrollController,
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
        itemCount: marineHours.length,
        separatorBuilder: (_, _) => const Divider(height: 1),
        itemBuilder: (context, i) => _SeaRow(point: marineHours[i]),
      );
    }
    if (atmHours.isEmpty) {
      return _EmptyPreset(label: context.l10n.weatherEmptyDay);
    }
    return ListView.separated(
      key: Key('preset-body-${preset.name}'),
      controller: scrollController,
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
      itemCount: atmHours.length,
      separatorBuilder: (_, _) => const Divider(height: 1),
      itemBuilder: (context, i) {
        final p = atmHours[i];
        return switch (preset) {
          _Preset.hourly => _HourlyRow(point: p),
          _Preset.wind => _WindRow(point: p),
          _Preset.precipitation => _PrecipRow(point: p),
          _Preset.sea => const SizedBox.shrink(),
        };
      },
    );
  }
}

bool _sameLocalDay(DateTime utc, DateTime localDay) {
  final l = utc.toLocal();
  return l.year == localDay.year &&
      l.month == localDay.month &&
      l.day == localDay.day;
}

String _hourLabel(DateTime utc) => DateFormat.j().format(utc.toLocal());

class _EmptyPreset extends StatelessWidget {
  final String label;
  const _EmptyPreset({required this.label});
  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Text(
          label,
          style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
        ),
      ),
    );
  }
}

class _HourlyRow extends StatelessWidget {
  final AtmosphericPoint point;
  const _HourlyRow({required this.point});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Row(
        children: [
          SizedBox(
            width: 56,
            child: Text(_hourLabel(point.timeUtc), style: textTheme.bodyMedium),
          ),
          WeatherSymbolIcon(symbol: point.symbol1h, size: 28),
          const SizedBox(width: 12),
          SizedBox(
            width: 48,
            child: Text(
              '${point.airTemperatureC.toStringAsFixed(0)}°',
              style: textTheme.titleSmall,
            ),
          ),
          const Spacer(),
          if ((point.precipitation1hMm ?? 0) > 0) ...[
            Text(
              '${point.precipitation1hMm!.toStringAsFixed(1)} mm',
              style: textTheme.bodySmall?.copyWith(color: colorScheme.primary),
            ),
            const SizedBox(width: 12),
          ],
          WindReadout(point: point),
        ],
      ),
    );
  }
}

class _WindRow extends StatelessWidget {
  final AtmosphericPoint point;
  const _WindRow({required this.point});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 10),
      child: Row(
        children: [
          SizedBox(
            width: 56,
            child: Text(_hourLabel(point.timeUtc), style: textTheme.bodyMedium),
          ),
          WindArrow(fromDeg: point.windFromDeg, size: 28),
          const SizedBox(width: 16),
          Text(
            '${point.windSpeedMs.toStringAsFixed(1)} m/s',
            style: textTheme.titleSmall,
          ),
          const Spacer(),
        ],
      ),
    );
  }
}

class _PrecipRow extends StatelessWidget {
  final AtmosphericPoint point;
  const _PrecipRow({required this.point});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    final precip = point.precipitation1hMm ?? 0;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 10),
      child: Row(
        children: [
          SizedBox(
            width: 56,
            child: Text(_hourLabel(point.timeUtc), style: textTheme.bodyMedium),
          ),
          Icon(
            point.isSnowing ? Icons.ac_unit : Icons.water_drop_outlined,
            size: 22,
            color: precip > 0
                ? colorScheme.primary
                : colorScheme.onSurfaceVariant.withValues(alpha: 0.5),
          ),
          const SizedBox(width: 16),
          Text(
            precip > 0 ? '${precip.toStringAsFixed(1)} mm' : '—',
            style: textTheme.titleSmall?.copyWith(
              color: precip > 0
                  ? colorScheme.primary
                  : colorScheme.onSurfaceVariant,
            ),
          ),
          const Spacer(),
          WeatherSymbolIcon(symbol: point.symbol1h, size: 22),
        ],
      ),
    );
  }
}

class _SeaRow extends StatelessWidget {
  final MarinePoint point;
  const _SeaRow({required this.point});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    final wave = point.waveHeightM;
    final water = point.seaWaterTemperatureC;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 10),
      child: Row(
        children: [
          SizedBox(
            width: 56,
            child: Text(_hourLabel(point.timeUtc), style: textTheme.bodyMedium),
          ),
          if (wave != null) ...[
            Icon(Icons.waves, size: 18, color: colorScheme.primary),
            const SizedBox(width: 4),
            Text('${wave.toStringAsFixed(1)} m', style: textTheme.titleSmall),
          ],
          const SizedBox(width: 16),
          if (point.waveFromDeg != null)
            WindArrow(fromDeg: point.waveFromDeg, size: 22),
          const Spacer(),
          if (water != null)
            Text('${water.toStringAsFixed(1)}°C',
                style: textTheme.titleSmall),
        ],
      ),
    );
  }
}

/// Inline sun / moon summary for the selected day. One row, no pills — just
/// icons with their time/value, matching the visual rhythm of the marker
/// info sheet's other detail rows.
class _SunRow extends StatelessWidget {
  final SunEvent sun;
  final MoonEvent? moon;
  const _SunRow({required this.sun, this.moon});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final scheme = Theme.of(context).colorScheme;
    final tt = Theme.of(context).textTheme;

    final segments = <_SunSegment>[];
    if (sun.polarDay) {
      segments.add(_SunSegment(
        icon: Icons.wb_sunny_outlined,
        text: l10n.weatherSunPolarDay,
      ));
    } else if (sun.polarNight) {
      segments.add(_SunSegment(
        icon: Icons.nightlight_outlined,
        text: l10n.weatherSunPolarNight,
      ));
    } else {
      if (sun.sunrise != null) {
        segments.add(_SunSegment(
          icon: Icons.wb_twilight,
          text: _hourMinute(sun.sunrise!),
        ));
      }
      if (sun.sunset != null) {
        segments.add(_SunSegment(
          icon: Icons.bedtime_outlined,
          text: _hourMinute(sun.sunset!),
        ));
      }
      final daylight = sun.daylight;
      if (daylight != null) {
        segments.add(_SunSegment(
          icon: Icons.timelapse,
          text: _formatDuration(daylight),
        ));
      }
    }
    final m = moon;
    if (m != null && m.illumination != null) {
      segments.add(_SunSegment(
        icon: Icons.brightness_2_outlined,
        text: '${(m.illumination! * 100).round()}%',
      ));
    }
    if (segments.isEmpty) return const SizedBox.shrink();

    return Padding(
      key: const Key('weather-detail-sun-row'),
      padding: const EdgeInsets.fromLTRB(20, 8, 20, 4),
      child: DefaultTextStyle.merge(
        style: tt.bodyMedium!.copyWith(color: scheme.onSurface),
        child: Wrap(
          spacing: 18,
          runSpacing: 4,
          children: [
            for (final seg in segments)
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(seg.icon,
                      size: 18, color: scheme.onSurfaceVariant),
                  const SizedBox(width: 6),
                  Text(seg.text),
                ],
              ),
          ],
        ),
      ),
    );
  }

  static String _hourMinute(DateTime t) {
    final local = t.toLocal();
    final h = local.hour.toString().padLeft(2, '0');
    final m = local.minute.toString().padLeft(2, '0');
    return '$h:$m';
  }

  static String _formatDuration(Duration d) {
    final h = d.inHours;
    final m = d.inMinutes.remainder(60);
    return '${h}h ${m.toString().padLeft(2, '0')}m';
  }
}

class _SunSegment {
  final IconData icon;
  final String text;
  const _SunSegment({required this.icon, required this.text});
}

class _AttributionFooter extends StatelessWidget {
  const _AttributionFooter();

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return SafeArea(
      top: false,
      child: InkWell(
        onTap: () => launchUrl(
          Uri.parse('https://www.met.no/en/free-meteorological-data'),
          mode: LaunchMode.externalApplication,
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          child: Text(
            l10n.weatherAttribution,
            style: textTheme.bodySmall
                ?.copyWith(color: colorScheme.onSurfaceVariant),
          ),
        ),
      ),
    );
  }
}
