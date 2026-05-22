import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';
import 'package:latlong2/latlong.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_selection_pill.dart';
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
/// Layout: drag handle → place header → embedded [EmbeddedWeatherBody]
/// (day strip + Weather/Ocean tabs) → attribution footer.
class WeatherDetailSheet extends StatelessWidget {
  final Marker marker;
  const WeatherDetailSheet({super.key, required this.marker});

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
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
                const BorderRadius.vertical(top: Radius.circular(AppRadius.xl)),
          ),
          child: Column(
            children: [
              const WeatherDragHandle(),
              _Header(title: marker.title),
              const SizedBox(height: AppSpacing.s),
              Expanded(
                child: EmbeddedWeatherBody(
                  position: marker.position,
                  scrollController: scrollController,
                ),
              ),
              const _AttributionFooter(),
            ],
          ),
        );
      },
    );
  }
}

/// The "guts" of the weather sheet — day strip, Weather/Ocean tab pills,
/// and the swipeable preset body. Designed to be embedded in any
/// container that supplies its own header (e.g. the long-press
/// [PinOptionsSheet] embeds this under its Weather tab).
class EmbeddedWeatherBody extends ConsumerStatefulWidget {
  final LatLng position;
  final ScrollController? scrollController;

  const EmbeddedWeatherBody({
    super.key,
    required this.position,
    this.scrollController,
  });

  @override
  ConsumerState<EmbeddedWeatherBody> createState() =>
      _EmbeddedWeatherBodyState();
}

enum _Preset { weather, ocean }

class _EmbeddedWeatherBodyState extends ConsumerState<EmbeddedWeatherBody>
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
    final forecast = ref.watch(weatherForecastProvider(widget.position));
    final tide = ref.watch(tideForecastProvider(widget.position));
    final tideValue = tide.maybeWhen(data: (v) => v, orElse: () => null);
    return forecast.when(
      loading: _loadingShell,
      error: (_, _) => _errorShell(context),
      data: (f) => _dataShell(context, f, tideValue),
    );
  }

  Widget _loadingShell() =>
      const Center(child: CircularProgressIndicator());

  Widget _errorShell(BuildContext context) {
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    return Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(AppSpacing.l),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.cloud_off_outlined, size: 48, color: colorScheme.error),
            const SizedBox(height: AppSpacing.m),
            Text(l10n.weatherLoadError),
            const SizedBox(height: AppSpacing.m),
            FilledButton(
              onPressed: () => ref
                  .read(weatherForecastProvider(widget.position).notifier)
                  .refresh(),
              child: Text(l10n.weatherRetry),
            ),
          ],
        ),
      ),
    );
  }

  Widget _dataShell(
      BuildContext context, WeatherForecast f, TideForecast? tide) {
    final daily = f.dailySummaries().take(9).toList();
    if (daily.isEmpty) return _errorShell(context);
    final clampedDay = _dayIndex.clamp(0, daily.length - 1);
    final presets = _availablePresets(f, tide);
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
        // _DragHandle / _Header are owned by the outer WeatherDetailSheet
        // shell; this Column is what gets embedded inside other sheets
        // (e.g. the pin sheet), so it must not render them itself.
        if (f.hasActiveAlerts)
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 4, 16, 4),
            child: MetAlertBanner(alert: f.topAlert!),
          ),
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 4, 16, 4),
          child: AvalancheWarningBadge(
            position: widget.position,
            currentAirTempC: f.currentAtmospheric?.airTemperatureC,
          ),
        ),
        if (sunForDay != null)
          _SunRow(sun: sunForDay, moon: moonForDay),
        const SizedBox(height: 4),
        _DayStrip(
          days: daily,
          selectedIndex: clampedDay,
          onSelect: (i) => setState(() => _dayIndex = i),
        ),
        const SizedBox(height: AppSpacing.s),
        _PresetTabs(
          presets: presets,
          selectedIndex: clampedPreset,
          onSelect: (i) {
            setState(() => _presetIndex = i);
            _presetController.animateToPage(
              i,
              duration: AppMotion.normal,
              curve: Curves.easeOut,
            );
          },
        ),
        const SizedBox(height: AppSpacing.xs),
        Expanded(
          child: PageView.builder(
            controller: _presetController,
            onPageChanged: (i) => setState(() => _presetIndex = i),
            itemCount: presets.length,
            itemBuilder: (context, i) => _PresetBody(
              preset: presets[i],
              forecast: f,
              tide: tide,
              day: daily[clampedDay].date,
              scrollController:
                  i == 0 ? widget.scrollController : null,
            ),
          ),
        ),
      ],
    );
  }

  List<_Preset> _availablePresets(WeatherForecast f, TideForecast? tide) => [
        _Preset.weather,
        // Ocean tab requires actual marine (waves / sea temp) data. Tide
        // alone — which Kartverket can return for marginal inland coords
        // near a station — isn't enough to justify the tab.
        if (f.hasMarineData) _Preset.ocean,
      ];
}

class WeatherDragHandle extends StatelessWidget {
  const WeatherDragHandle({super.key});
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
            borderRadius: BorderRadius.circular(AppRadius.s),
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
        padding: const EdgeInsets.symmetric(horizontal: AppSpacing.l),
        itemCount: days.length,
        separatorBuilder: (_, _) => const SizedBox(width: AppSpacing.s),
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
        ? colorScheme.primary
        : colorScheme.surfaceContainerHigh;
    final fg = selected ? colorScheme.onPrimary : colorScheme.onSurface;
    final dayLabel = DateFormat('EEE d').format(summary.date);
    return Material(
      color: bg,
      borderRadius: BorderRadius.circular(AppRadius.pill),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTap,
        child: Container(
          width: 84,
          padding: const EdgeInsets.symmetric(vertical: AppSpacing.s),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Text(dayLabel,
                  style: textTheme.bodyMedium?.copyWith(color: fg)),
              const SizedBox(height: AppSpacing.xs),
              WeatherSymbolIcon(symbol: summary.middaySymbol, size: 28),
              const SizedBox(height: AppSpacing.xs),
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
        padding: const EdgeInsets.symmetric(horizontal: AppSpacing.l),
        itemCount: presets.length,
        separatorBuilder: (_, _) => const SizedBox(width: AppSpacing.s),
        itemBuilder: (context, i) {
          final preset = presets[i];
          return AppSelectionPill(
            key: Key('preset-chip-${preset.name}'),
            selected: i == selectedIndex,
            onTap: () => onSelect(i),
            leadingIcon: _iconFor(preset),
            child: Text(_labelFor(context, preset)),
          );
        },
      ),
    );
  }

  IconData _iconFor(_Preset preset) {
    return switch (preset) {
      _Preset.weather => Icons.wb_sunny_outlined,
      _Preset.ocean => Icons.sailing_outlined,
    };
  }

  String _labelFor(BuildContext context, _Preset preset) {
    final l10n = context.l10n;
    return switch (preset) {
      _Preset.weather => l10n.weatherTabWeather,
      _Preset.ocean => l10n.weatherTabOcean,
    };
  }
}

class _PresetBody extends StatelessWidget {
  final _Preset preset;
  final WeatherForecast forecast;
  final TideForecast? tide;
  final DateTime day;
  final ScrollController? scrollController;
  const _PresetBody({
    required this.preset,
    required this.forecast,
    required this.tide,
    required this.day,
    required this.scrollController,
  });

  @override
  Widget build(BuildContext context) {
    if (preset == _Preset.ocean) {
      return _OceanBody(
        forecast: forecast,
        tide: tide,
        day: day,
        scrollController: scrollController,
      );
    }
    final atmHours = forecast.atmospheric
        .where((p) => _sameLocalDay(p.timeUtc, day))
        .toList();
    if (atmHours.isEmpty) {
      return _EmptyPreset(label: context.l10n.weatherEmptyDay);
    }
    return ListView.separated(
      key: Key('preset-body-${preset.name}'),
      controller: scrollController,
      padding: const EdgeInsets.fromLTRB(
          AppSpacing.l, AppSpacing.s, AppSpacing.l, AppSpacing.s),
      itemCount: atmHours.length,
      separatorBuilder: (_, _) => const Divider(height: 1),
      itemBuilder: (context, i) => _WeatherRow(point: atmHours[i]),
    );
  }
}

class _OceanBody extends StatelessWidget {
  final WeatherForecast forecast;
  final TideForecast? tide;
  final DateTime day;
  final ScrollController? scrollController;

  const _OceanBody({
    required this.forecast,
    required this.tide,
    required this.day,
    required this.scrollController,
  });

  @override
  Widget build(BuildContext context) {
    final marineHours =
        forecast.marine.where((p) => _sameLocalDay(p.timeUtc, day)).toList();
    final tideRows = tide?.forLocalDay(day) ?? const <TideExtremum>[];

    if (marineHours.isEmpty && tideRows.isEmpty) {
      return _EmptyPreset(label: context.l10n.weatherMarineEmpty);
    }

    return ListView(
      key: const Key('preset-body-ocean'),
      controller: scrollController,
      padding: const EdgeInsets.fromLTRB(
          AppSpacing.l, AppSpacing.s, AppSpacing.l, AppSpacing.s),
      children: [
        _TideCard(tideRows: tideRows, tideAvailable: tide != null),
        if (marineHours.isNotEmpty) ...[
          const SizedBox(height: AppSpacing.m),
          for (int i = 0; i < marineHours.length; i++) ...[
            _OceanRow(point: marineHours[i]),
            if (i < marineHours.length - 1) const Divider(height: 1),
          ],
        ],
      ],
    );
  }
}

class _TideCard extends StatelessWidget {
  final List<TideExtremum> tideRows;
  final bool tideAvailable;
  const _TideCard({required this.tideRows, required this.tideAvailable});

  @override
  Widget build(BuildContext context) {
    if (!tideAvailable) {
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: AppSpacing.s),
        child: Text(
          context.l10n.weatherTideNoData,
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
        ),
      );
    }
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;
    return Container(
      key: const Key('weather-detail-tide-card'),
      padding: const EdgeInsets.all(AppSpacing.m),
      decoration: BoxDecoration(
        color: colorScheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(AppRadius.l),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(Icons.show_chart, size: 18),
              const SizedBox(width: AppSpacing.s),
              Text(context.l10n.weatherTideLabel,
                  style: textTheme.titleSmall),
            ],
          ),
          const SizedBox(height: AppSpacing.s),
          if (tideRows.isEmpty)
            Text(context.l10n.weatherMarineEmpty,
                style: textTheme.bodySmall?.copyWith(
                  color: colorScheme.onSurfaceVariant,
                ))
          else
            for (final row in tideRows)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 2),
                child: Row(
                  children: [
                    Icon(tideBucketIcon(row.kind),
                        size: 18, color: colorScheme.primary),
                    const SizedBox(width: AppSpacing.s),
                    SizedBox(
                      width: 56,
                      child: Text(
                        row.kind == TideKind.high
                            ? context.l10n.weatherTideHigh
                            : context.l10n.weatherTideLow,
                        style: textTheme.bodyMedium,
                      ),
                    ),
                    Text(_hourLabel(row.timeUtc), style: textTheme.bodyMedium),
                    const Spacer(),
                    Text('${row.levelCm.toStringAsFixed(0)} cm',
                        style: textTheme.titleSmall),
                  ],
                ),
              ),
        ],
      ),
    );
  }
}

IconData tideBucketIcon(TideKind kind) =>
    kind == TideKind.high ? Icons.arrow_upward : Icons.arrow_downward;

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
        padding: const EdgeInsets.all(AppSpacing.xl),
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

class _WeatherRow extends StatelessWidget {
  final AtmosphericPoint point;
  const _WeatherRow({required this.point});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    final precip = point.precipitation1hMm ?? 0;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.s),
      child: Row(
        children: [
          SizedBox(
            width: 56,
            child: Text(_hourLabel(point.timeUtc), style: textTheme.bodyMedium),
          ),
          WeatherSymbolIcon(symbol: point.symbol1h, size: 28),
          const SizedBox(width: AppSpacing.m),
          SizedBox(
            width: 48,
            child: Text(
              '${point.airTemperatureC.toStringAsFixed(0)}°',
              style: textTheme.titleSmall,
            ),
          ),
          const Spacer(),
          if (precip > 0) ...[
            Icon(
              precipBucketIcon(precip, snow: point.isSnowing),
              size: 18,
              color: colorScheme.primary,
            ),
            const SizedBox(width: AppSpacing.xs),
            Text(
              '${precip.toStringAsFixed(1)} mm',
              style: textTheme.bodySmall?.copyWith(color: colorScheme.primary),
            ),
            const SizedBox(width: AppSpacing.m),
          ],
          WindArrow(
            fromDeg: point.windFromDeg,
            size: windArrowSize(point.windSpeedMs),
          ),
          const SizedBox(width: AppSpacing.xs),
          Text('${point.windSpeedMs.toStringAsFixed(1)} m/s',
              style: textTheme.bodyMedium),
        ],
      ),
    );
  }
}

class _OceanRow extends StatelessWidget {
  final MarinePoint point;
  const _OceanRow({required this.point});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    final wave = point.waveHeightM;
    final water = point.seaWaterTemperatureC;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.s + 2),
      child: Row(
        children: [
          SizedBox(
            width: 56,
            child: Text(_hourLabel(point.timeUtc), style: textTheme.bodyMedium),
          ),
          if (wave != null) ...[
            Icon(
              waveBucketIcon(wave),
              size: wave < 0.5 ? 18 : (wave < 2.0 ? 22 : 26),
              color: colorScheme.primary,
            ),
            const SizedBox(width: AppSpacing.xs),
            Text('${wave.toStringAsFixed(1)} m', style: textTheme.titleSmall),
          ],
          const SizedBox(width: AppSpacing.l),
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
          padding: const EdgeInsets.symmetric(
              horizontal: AppSpacing.l, vertical: AppSpacing.s),
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
