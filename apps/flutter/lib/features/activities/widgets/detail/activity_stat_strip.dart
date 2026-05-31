import 'package:flutter/material.dart';

/// One stat in [ActivityStatStrip]. Pure data — both label and value
/// are short strings, no nullables. Strip omits a cell when its value
/// is null so kinds can show 2-, 3- or 4-stat layouts from the same
/// list.
class StatItem {
  final String label;
  final String? value;
  const StatItem(this.label, this.value);
}

/// Equal-flex stat strip. Identical type rhythm across every kind:
/// uppercase 10/14 label with 0.8 letter-spacing on top, 15/20 medium
/// value below. Vertical 1px dividers between cells in
/// `outlineVariant`. No icons, no color — this is the calm middle of
/// the detail sheet.
class ActivityStatStrip extends StatelessWidget {
  final List<StatItem> items;
  const ActivityStatStrip({super.key, required this.items});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final filled = items.where((s) => s.value != null && s.value!.isNotEmpty).toList(growable: false);
    if (filled.isEmpty) return const SizedBox.shrink();
    return Container(
      margin: const EdgeInsets.only(top: 12),
      padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 10),
      decoration: BoxDecoration(
        color: theme.colorScheme.surfaceContainer,
        borderRadius: BorderRadius.circular(14),
      ),
      child: IntrinsicHeight(
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (var i = 0; i < filled.length; i++) ...[
              Expanded(child: _Cell(item: filled[i])),
              if (i < filled.length - 1)
                Container(
                  width: 1,
                  margin: const EdgeInsets.symmetric(vertical: 4),
                  color: theme.colorScheme.outlineVariant,
                ),
            ],
          ],
        ),
      ),
    );
  }
}

class _Cell extends StatelessWidget {
  final StatItem item;
  const _Cell({required this.item});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 4),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Text(
            item.label.toUpperCase(),
            textAlign: TextAlign.center,
            style: TextStyle(
              fontSize: 10,
              height: 14 / 10,
              fontWeight: FontWeight.w500,
              letterSpacing: 0.8,
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: 2),
          Text(
            item.value ?? '—',
            textAlign: TextAlign.center,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              fontSize: 15,
              height: 20 / 15,
              fontWeight: FontWeight.w500,
              color: theme.colorScheme.onSurface,
            ),
          ),
        ],
      ),
    );
  }
}
