import 'package:flutter/material.dart';

import '../../models/activity_analysis.dart';
import 'forecast_band_row.dart';

/// Per-driver card: label, current value, unit, weight-vs-others bar,
/// confidence pips, expandable rationale, and an embedded sparkline when
/// the driver carries a forecast band. The whole detail screen renders as
/// a list of these (sorted by weight descending), so the user sees *why*
/// the score is what it is, not just a bare badge.
class DriverCard extends StatelessWidget {
  final AnalysisDriver driver;
  final Color tintColor;

  const DriverCard({super.key, required this.driver, required this.tintColor});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final hasRationale = driver.rationale != null && driver.rationale!.isNotEmpty;
    final body = Padding(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  driver.label,
                  style: theme.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
              _ConfidenceDots(confidence: driver.confidence),
            ],
          ),
          const SizedBox(height: 6),
          Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text(
                _valueText(driver),
                style: theme.textTheme.headlineSmall?.copyWith(
                  color: tintColor,
                  fontWeight: FontWeight.w700,
                ),
              ),
              if (driver.direction != null) ...[
                const SizedBox(width: 8),
                Padding(
                  padding: const EdgeInsets.only(bottom: 4),
                  child: Text(
                    driver.direction!,
                    style: theme.textTheme.bodySmall,
                  ),
                ),
              ],
            ],
          ),
          const SizedBox(height: 8),
          _WeightBar(weight: driver.weight, tintColor: tintColor),
          if (driver.band != null) ...[
            const SizedBox(height: 10),
            ForecastBandRow(band: driver.band!, tintColor: tintColor),
          ],
          if (hasRationale) ...[
            const SizedBox(height: 8),
            Text(
              driver.rationale!,
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ],
      ),
    );

    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
      child: body,
    );
  }

  static String _valueText(AnalysisDriver d) {
    if (d.value == null) return '—';
    final v = d.value!;
    final asInt = v.truncateToDouble() == v;
    final number = asInt ? v.toInt().toString() : v.toStringAsFixed(1);
    return d.unit == null ? number : '$number ${d.unit}';
  }
}

class _ConfidenceDots extends StatelessWidget {
  final double confidence; // 0..1
  const _ConfidenceDots({required this.confidence});

  @override
  Widget build(BuildContext context) {
    final filled = (confidence * 3).round().clamp(0, 3);
    final color = Theme.of(context).colorScheme.primary;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: List.generate(3, (i) {
        final on = i < filled;
        return Padding(
          padding: const EdgeInsets.symmetric(horizontal: 1.5),
          child: Container(
            width: 7,
            height: 7,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: on ? color : color.withValues(alpha: 0.18),
            ),
          ),
        );
      }),
    );
  }
}

class _WeightBar extends StatelessWidget {
  final double weight; // expected 0..1
  final Color tintColor;
  const _WeightBar({required this.weight, required this.tintColor});

  @override
  Widget build(BuildContext context) {
    final clamped = weight.clamp(0.0, 1.0);
    return ClipRRect(
      borderRadius: BorderRadius.circular(4),
      child: LinearProgressIndicator(
        value: clamped,
        minHeight: 4,
        backgroundColor: tintColor.withValues(alpha: 0.1),
        valueColor: AlwaysStoppedAnimation<Color>(tintColor.withValues(alpha: 0.7)),
      ),
    );
  }
}
