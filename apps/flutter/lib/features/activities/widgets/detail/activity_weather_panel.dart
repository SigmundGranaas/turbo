import 'package:flutter/material.dart';

/// One small metric in the right-hand column of the weather header.
class WeatherMetric {
  final IconData icon;
  final String value;
  const WeatherMetric(this.icon, this.value);
}

/// One hour cell in the 6h micro-strip below the headline.
class WeatherHour {
  final String time;
  final IconData symbol;
  final String temperature;
  final String? subline;
  const WeatherHour({
    required this.time,
    required this.symbol,
    required this.temperature,
    this.subline,
  });
}

/// Single card showing the headline weather metric (large temperature
/// + symbol + brief summary), a 2×2 metrics grid on the right, and an
/// optional 6-hour micro-strip below. An optional `extra` slot at the
/// bottom can carry kind-specific callouts (rain-arrival warning,
/// pressure-rising chip, etc.).
///
/// Built to render gracefully when conditions are still loading: pass
/// [WeatherLoadingState.loading] / `.error` and the panel keeps its
/// shape with a quiet skeletal placeholder instead of an animated
/// spinner.
class ActivityWeatherPanel extends StatelessWidget {
  final String title;
  final String? subtitle;
  final Color accent;
  final WeatherSummary? summary;
  final WeatherLoadingState loadingState;
  final List<WeatherHour> hourly;
  final Widget? extra;
  final VoidCallback? onRefresh;

  const ActivityWeatherPanel({
    super.key,
    required this.title,
    required this.accent,
    this.subtitle,
    this.summary,
    this.loadingState = WeatherLoadingState.ready,
    this.hourly = const [],
    this.extra,
    this.onRefresh,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      margin: const EdgeInsets.only(top: 12),
      padding: const EdgeInsets.fromLTRB(14, 12, 14, 14),
      decoration: BoxDecoration(
        color: theme.colorScheme.surfaceContainer,
        borderRadius: BorderRadius.circular(14),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _Header(
            title: title,
            subtitle: subtitle,
            onRefresh: onRefresh,
          ),
          const SizedBox(height: 10),
          switch (loadingState) {
            WeatherLoadingState.loading => _LoadingBody(accent: accent),
            WeatherLoadingState.error => _ErrorBody(
                icon: Icons.cloud_off_outlined,
                message: 'Conditions unavailable.',
                action: onRefresh == null ? null : 'Tap refresh to retry.',
              ),
            WeatherLoadingState.noData => const _ErrorBody(
                icon: Icons.sentiment_neutral_outlined,
                message: 'No weather drivers reported for this spot yet.',
              ),
            WeatherLoadingState.ready => summary == null
                ? const _ErrorBody(
                    icon: Icons.sentiment_neutral_outlined,
                    message: 'No weather drivers reported for this spot yet.',
                  )
                : _ReadyBody(summary: summary!, hourly: hourly, accent: accent),
          },
          if (extra != null) ...[
            const SizedBox(height: 10),
            extra!,
          ],
        ],
      ),
    );
  }
}

/// What the weather panel is doing right now. `noData` is a distinct
/// state from `error` — the analysis call succeeded but didn't carry
/// any drivers the panel can render, which is honest user-facing copy
/// versus a misleading "pull to retry".
enum WeatherLoadingState { loading, ready, error, noData }

class WeatherSummary {
  final IconData symbol;
  final String temperature;
  final String summary;
  final List<WeatherMetric> metrics;
  const WeatherSummary({
    required this.symbol,
    required this.temperature,
    required this.summary,
    required this.metrics,
  });
}

class _Header extends StatelessWidget {
  final String title;
  final String? subtitle;
  final VoidCallback? onRefresh;
  const _Header({required this.title, this.subtitle, this.onRefresh});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      children: [
        Text(
          title.toUpperCase(),
          style: TextStyle(
            fontSize: 11,
            height: 14 / 11,
            fontWeight: FontWeight.w500,
            letterSpacing: 0.8,
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
        const Spacer(),
        if (subtitle != null)
          Text(
            subtitle!,
            style: TextStyle(
              fontSize: 11,
              height: 14 / 11,
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        if (onRefresh != null) ...[
          const SizedBox(width: 6),
          InkResponse(
            onTap: onRefresh,
            radius: 14,
            child: Padding(
              padding: const EdgeInsets.all(2),
              child: Icon(
                Icons.refresh,
                size: 14,
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ),
        ],
      ],
    );
  }
}

class _ReadyBody extends StatelessWidget {
  final WeatherSummary summary;
  final List<WeatherHour> hourly;
  final Color accent;
  const _ReadyBody({
    required this.summary,
    required this.hourly,
    required this.accent,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Icon(summary.symbol, size: 40, color: accent),
            const SizedBox(width: 14),
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  summary.temperature,
                  style: TextStyle(
                    fontSize: 30,
                    height: 34 / 30,
                    color: theme.colorScheme.onSurface,
                  ),
                ),
                Text(
                  summary.summary,
                  style: TextStyle(
                    fontSize: 12,
                    height: 16 / 12,
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
              ],
            ),
            const Spacer(),
            if (summary.metrics.isNotEmpty)
              _MetricsGrid(metrics: summary.metrics),
          ],
        ),
        if (hourly.isNotEmpty) ...[
          const SizedBox(height: 12),
          _HourStrip(hourly: hourly, accent: accent),
        ],
      ],
    );
  }
}

class _MetricsGrid extends StatelessWidget {
  final List<WeatherMetric> metrics;
  const _MetricsGrid({required this.metrics});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 150),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.end,
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final m in metrics.take(4))
            Padding(
              padding: const EdgeInsets.only(bottom: 2),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(m.icon, size: 13, color: theme.colorScheme.onSurfaceVariant),
                  const SizedBox(width: 4),
                  Text(
                    m.value,
                    style: TextStyle(
                      fontSize: 12,
                      height: 18 / 12,
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

class _HourStrip extends StatelessWidget {
  final List<WeatherHour> hourly;
  final Color accent;
  const _HourStrip({required this.hourly, required this.accent});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      children: [
        for (var i = 0; i < hourly.length; i++)
          Expanded(
            child: Padding(
              padding: EdgeInsets.only(left: i == 0 ? 0 : 4),
              child: Container(
                padding: const EdgeInsets.symmetric(vertical: 6),
                decoration: BoxDecoration(
                  color: i == 0 ? accent.withValues(alpha: 0.10) : Colors.transparent,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Column(
                  children: [
                    Text(
                      hourly[i].time,
                      style: TextStyle(
                        fontSize: 11,
                        height: 14 / 11,
                        fontWeight: FontWeight.w500,
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Icon(hourly[i].symbol, size: 18, color: theme.colorScheme.onSurface),
                    const SizedBox(height: 2),
                    Text(
                      hourly[i].temperature,
                      style: TextStyle(
                        fontSize: 12,
                        height: 16 / 12,
                        fontWeight: FontWeight.w500,
                        color: theme.colorScheme.onSurface,
                      ),
                    ),
                    if (hourly[i].subline != null)
                      Text(
                        hourly[i].subline!,
                        style: TextStyle(
                          fontSize: 10,
                          height: 12 / 10,
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
      ],
    );
  }
}

class _LoadingBody extends StatelessWidget {
  final Color accent;
  const _LoadingBody({required this.accent});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    // Static placeholder. No animation — the user explicitly noted
    // infinite-shimmer panels made the app feel broken.
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        Icon(Icons.cloud_outlined, size: 40, color: accent.withValues(alpha: 0.6)),
        const SizedBox(width: 14),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 80,
                height: 16,
                decoration: BoxDecoration(
                  color: theme.colorScheme.surfaceContainerHighest,
                  borderRadius: BorderRadius.circular(4),
                ),
              ),
              const SizedBox(height: 6),
              Container(
                width: 140,
                height: 10,
                decoration: BoxDecoration(
                  color: theme.colorScheme.surfaceContainerHighest,
                  borderRadius: BorderRadius.circular(4),
                ),
              ),
              const SizedBox(height: 4),
              Text(
                'Fetching conditions…',
                style: TextStyle(
                  fontSize: 11,
                  height: 14 / 11,
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _ErrorBody extends StatelessWidget {
  final IconData icon;
  final String message;
  final String? action;
  const _ErrorBody({
    required this.icon,
    required this.message,
    this.action,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        Icon(icon, size: 32, color: theme.colorScheme.onSurfaceVariant),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                message,
                style: TextStyle(
                  fontSize: 13,
                  height: 18 / 13,
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
              if (action != null)
                Text(
                  action!,
                  style: TextStyle(
                    fontSize: 11,
                    height: 14 / 11,
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
            ],
          ),
        ),
      ],
    );
  }
}
