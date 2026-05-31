import 'package:flutter/material.dart';

import 'package:turbo/features/activities/api.dart' show ObservationDraft;

const _tintColor = Color(0xFF1565C0);

/// Freediving-specific observation extras. The headline field is
/// `visibilityMeters` — direct ground-truth for the orchestrator's
/// computed viz estimate. Lets users say "I saw 4 m" instead of
/// "typical visibility 5 m" forever.
class FreedivingObservationExtras extends StatefulWidget {
  final ObservationDraft draft;
  final VoidCallback onChanged;

  const FreedivingObservationExtras({
    super.key,
    required this.draft,
    required this.onChanged,
  });

  @override
  State<FreedivingObservationExtras> createState() => _FreedivingObservationExtrasState();
}

class _FreedivingObservationExtrasState extends State<FreedivingObservationExtras> {
  static const _currentOptions = <(String, String)>[
    ('none', 'No current'),
    ('light', 'Light'),
    ('moderate', 'Moderate'),
    ('strong', 'Strong'),
  ];

  final _vizCtrl = TextEditingController();
  final _waterTempCtrl = TextEditingController();
  final _concernsCtrl = TextEditingController();
  final _speciesCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    _vizCtrl.text = widget.draft.kindPayload['visibilityMeters']?.toString() ?? '';
    _waterTempCtrl.text = widget.draft.kindPayload['waterTempC']?.toString() ?? '';
    _concernsCtrl.text = widget.draft.kindPayload['concerns'] as String? ?? '';
    final species = widget.draft.kindPayload['speciesSeen'] as List<dynamic>?;
    _speciesCtrl.text = (species ?? const []).join(', ');
  }

  @override
  void dispose() {
    _vizCtrl.dispose();
    _waterTempCtrl.dispose();
    _concernsCtrl.dispose();
    _speciesCtrl.dispose();
    super.dispose();
  }

  String? _current() => widget.draft.kindPayload['currentStrength'] as String?;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Visibility (m)', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 6),
        TextField(
          controller: _vizCtrl,
          keyboardType: const TextInputType.numberWithOptions(decimal: true),
          decoration: const InputDecoration(
            hintText: 'e.g. 6',
            border: OutlineInputBorder(),
          ),
          onChanged: (v) {
            final n = double.tryParse(v.replaceAll(',', '.'));
            if (n == null) {
              widget.draft.kindPayload.remove('visibilityMeters');
            } else {
              widget.draft.kindPayload['visibilityMeters'] = n;
            }
            widget.onChanged();
          },
        ),
        const SizedBox(height: 16),
        Text('Water temperature (°C)', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 4),
        TextField(
          controller: _waterTempCtrl,
          keyboardType: const TextInputType.numberWithOptions(decimal: true, signed: true),
          decoration: const InputDecoration(border: OutlineInputBorder()),
          onChanged: (v) {
            final n = double.tryParse(v.replaceAll(',', '.'));
            if (n == null) {
              widget.draft.kindPayload.remove('waterTempC');
            } else {
              widget.draft.kindPayload['waterTempC'] = n;
            }
            widget.onChanged();
          },
        ),
        const SizedBox(height: 16),
        Text('Current strength', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 6),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            for (final (key, label) in _currentOptions)
              ChoiceChip(
                label: Text(label),
                selected: _current() == key,
                selectedColor: _tintColor.withValues(alpha: 0.20),
                onSelected: (s) {
                  setState(() {
                    if (s) {
                      widget.draft.kindPayload['currentStrength'] = key;
                    } else {
                      widget.draft.kindPayload.remove('currentStrength');
                    }
                  });
                  widget.onChanged();
                },
              ),
          ],
        ),
        const SizedBox(height: 16),
        Text('Species seen (comma-separated)',
            style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 4),
        TextField(
          controller: _speciesCtrl,
          decoration: const InputDecoration(
            hintText: 'cod, mackerel, ...',
            border: OutlineInputBorder(),
          ),
          onChanged: (v) {
            final parts = v
                .split(',')
                .map((s) => s.trim())
                .where((s) => s.isNotEmpty)
                .toList();
            if (parts.isEmpty) {
              widget.draft.kindPayload.remove('speciesSeen');
            } else {
              widget.draft.kindPayload['speciesSeen'] = parts;
            }
            widget.onChanged();
          },
        ),
        const SizedBox(height: 12),
        Text('Concerns (optional)',
            style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 4),
        TextField(
          controller: _concernsCtrl,
          decoration: const InputDecoration(border: OutlineInputBorder()),
          minLines: 1,
          maxLines: 3,
          onChanged: (v) {
            final trimmed = v.trim();
            if (trimmed.isEmpty) {
              widget.draft.kindPayload.remove('concerns');
            } else {
              widget.draft.kindPayload['concerns'] = trimmed;
            }
            widget.onChanged();
          },
        ),
      ],
    );
  }
}
