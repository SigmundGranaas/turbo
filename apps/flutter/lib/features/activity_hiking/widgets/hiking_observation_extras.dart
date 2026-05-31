import 'package:flutter/material.dart';

import 'package:turbo/features/activities/api.dart' show ObservationDraft;

const _tintColor = Color(0xFF2E7D32);

/// Hiking observation extras — surfaces trail condition codes, the
/// elevation snow line if encountered, water-source confirmation, and
/// marking state.
class HikingObservationExtras extends StatefulWidget {
  final ObservationDraft draft;
  final VoidCallback onChanged;

  const HikingObservationExtras({
    super.key,
    required this.draft,
    required this.onChanged,
  });

  @override
  State<HikingObservationExtras> createState() => _HikingObservationExtrasState();
}

class _HikingObservationExtrasState extends State<HikingObservationExtras> {
  static const _condOptions = <(String, String)>[
    ('dry', 'Dry'),
    ('muddy', 'Muddy'),
    ('snow_patches', 'Snow patches'),
    ('windy_above_treeline', 'Wind above treeline'),
    ('fords_difficult', 'Difficult fords'),
  ];

  static const _markingOptions = <(String, String)>[
    ('clear', 'Clearly marked'),
    ('faded', 'Faded'),
    ('missing', 'Missing in spots'),
  ];

  final _snowAtCtrl = TextEditingController();
  final _concernsCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    _snowAtCtrl.text = widget.draft.kindPayload['snowAt']?.toString() ?? '';
    _concernsCtrl.text = widget.draft.kindPayload['concerns'] as String? ?? '';
  }

  @override
  void dispose() {
    _snowAtCtrl.dispose();
    _concernsCtrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final cond = widget.draft.kindPayload['trailCondition'] as String?;
    final marking = widget.draft.kindPayload['markingState'] as String?;
    final water = widget.draft.kindPayload['waterSourcesFlowing'] as bool?;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Trail condition', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 6),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            for (final (key, label) in _condOptions)
              ChoiceChip(
                label: Text(label),
                selected: cond == key,
                selectedColor: _tintColor.withValues(alpha: 0.20),
                onSelected: (s) {
                  setState(() {
                    if (s) {
                      widget.draft.kindPayload['trailCondition'] = key;
                    } else {
                      widget.draft.kindPayload.remove('trailCondition');
                    }
                  });
                  widget.onChanged();
                },
              ),
          ],
        ),
        const SizedBox(height: 16),
        Text('Snow line at elevation (m, optional)',
            style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 4),
        TextField(
          controller: _snowAtCtrl,
          keyboardType: TextInputType.number,
          decoration: const InputDecoration(border: OutlineInputBorder()),
          onChanged: (v) {
            final n = double.tryParse(v.replaceAll(',', '.'));
            if (n == null) {
              widget.draft.kindPayload.remove('snowAt');
            } else {
              widget.draft.kindPayload['snowAt'] = n;
            }
            widget.onChanged();
          },
        ),
        const SizedBox(height: 16),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: const Text('Water sources flowing'),
          value: water ?? false,
          activeThumbColor: _tintColor,
          onChanged: (b) {
            setState(() => widget.draft.kindPayload['waterSourcesFlowing'] = b);
            widget.onChanged();
          },
        ),
        const SizedBox(height: 8),
        Text('Marking', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 6),
        Wrap(
          spacing: 8,
          children: [
            for (final (key, label) in _markingOptions)
              ChoiceChip(
                label: Text(label),
                selected: marking == key,
                selectedColor: _tintColor.withValues(alpha: 0.20),
                onSelected: (s) {
                  setState(() {
                    if (s) {
                      widget.draft.kindPayload['markingState'] = key;
                    } else {
                      widget.draft.kindPayload.remove('markingState');
                    }
                  });
                  widget.onChanged();
                },
              ),
          ],
        ),
        const SizedBox(height: 12),
        TextField(
          controller: _concernsCtrl,
          decoration: const InputDecoration(
            labelText: 'Concerns', border: OutlineInputBorder()),
          minLines: 1,
          maxLines: 3,
          onChanged: (v) {
            final t = v.trim();
            if (t.isEmpty) {
              widget.draft.kindPayload.remove('concerns');
            } else {
              widget.draft.kindPayload['concerns'] = t;
            }
            widget.onChanged();
          },
        ),
      ],
    );
  }
}
