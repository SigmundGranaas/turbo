import 'package:flutter/material.dart';

import 'package:turbo/features/activities/api.dart' show ObservationDraft;

const _tintColor = Color(0xFF00838F);

/// Packrafting observation extras. Surfaces observed grade vs the
/// route's typical grade, water temperature actually felt, portages
/// taken, hazards.
class PackraftingObservationExtras extends StatefulWidget {
  final ObservationDraft draft;
  final VoidCallback onChanged;

  const PackraftingObservationExtras({
    super.key,
    required this.draft,
    required this.onChanged,
  });

  @override
  State<PackraftingObservationExtras> createState() =>
      _PackraftingObservationExtrasState();
}

class _PackraftingObservationExtrasState
    extends State<PackraftingObservationExtras> {
  static const _gradeOptions = <(String, String)>[
    ('I', 'I'), ('II', 'II'), ('III', 'III'), ('IV', 'IV'),
    ('V', 'V'), ('VI', 'VI'),
  ];

  static const _hazardOptions = <(String, String)>[
    ('strainer', 'Strainer / wood'),
    ('undercut', 'Undercut'),
    ('hole', 'Hydraulic'),
    ('sweeper', 'Sweeper'),
    ('low_bridge', 'Low bridge'),
  ];

  final _waterTempCtrl = TextEditingController();
  final _flowCtrl = TextEditingController();
  final _portagesCtrl = TextEditingController();
  final _concernsCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    _waterTempCtrl.text = widget.draft.kindPayload['waterTempC']?.toString() ?? '';
    _flowCtrl.text = widget.draft.kindPayload['flowCumecs']?.toString() ?? '';
    _portagesCtrl.text = widget.draft.kindPayload['portagesTaken']?.toString() ?? '';
    _concernsCtrl.text = widget.draft.kindPayload['concerns'] as String? ?? '';
  }

  @override
  void dispose() {
    _waterTempCtrl.dispose();
    _flowCtrl.dispose();
    _portagesCtrl.dispose();
    _concernsCtrl.dispose();
    super.dispose();
  }

  Set<String> _hazards() {
    final raw = widget.draft.kindPayload['hazardsNoted'];
    if (raw is List) return raw.cast<String>().toSet();
    return {};
  }

  @override
  Widget build(BuildContext context) {
    final grade = widget.draft.kindPayload['observedGrade'] as String?;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Observed grade', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 6),
        Wrap(
          spacing: 6,
          children: [
            for (final (key, label) in _gradeOptions)
              ChoiceChip(
                label: Text(label),
                selected: grade == key,
                selectedColor: _tintColor.withValues(alpha: 0.20),
                onSelected: (s) {
                  setState(() {
                    if (s) {
                      widget.draft.kindPayload['observedGrade'] = key;
                    } else {
                      widget.draft.kindPayload.remove('observedGrade');
                    }
                  });
                  widget.onChanged();
                },
              ),
          ],
        ),
        const SizedBox(height: 16),
        Row(children: [
          Expanded(
            child: TextField(
              controller: _waterTempCtrl,
              keyboardType: const TextInputType.numberWithOptions(decimal: true, signed: true),
              decoration: const InputDecoration(
                labelText: 'Water temp (°C)', border: OutlineInputBorder()),
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
          ),
          const SizedBox(width: 8),
          Expanded(
            child: TextField(
              controller: _flowCtrl,
              keyboardType: const TextInputType.numberWithOptions(decimal: true),
              decoration: const InputDecoration(
                labelText: 'Flow (m³/s)', border: OutlineInputBorder()),
              onChanged: (v) {
                final n = double.tryParse(v.replaceAll(',', '.'));
                if (n == null) {
                  widget.draft.kindPayload.remove('flowCumecs');
                } else {
                  widget.draft.kindPayload['flowCumecs'] = n;
                }
                widget.onChanged();
              },
            ),
          ),
        ]),
        const SizedBox(height: 8),
        TextField(
          controller: _portagesCtrl,
          keyboardType: TextInputType.number,
          decoration: const InputDecoration(
            labelText: 'Portages taken', border: OutlineInputBorder()),
          onChanged: (v) {
            final n = int.tryParse(v);
            if (n == null) {
              widget.draft.kindPayload.remove('portagesTaken');
            } else {
              widget.draft.kindPayload['portagesTaken'] = n;
            }
            widget.onChanged();
          },
        ),
        const SizedBox(height: 16),
        Text('Hazards noted', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 6),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            for (final (key, label) in _hazardOptions)
              FilterChip(
                label: Text(label),
                selected: _hazards().contains(key),
                selectedColor: Colors.red.shade400.withValues(alpha: 0.25),
                onSelected: (s) {
                  setState(() {
                    final h = _hazards();
                    if (s) {
                      h.add(key);
                    } else {
                      h.remove(key);
                    }
                    if (h.isEmpty) {
                      widget.draft.kindPayload.remove('hazardsNoted');
                    } else {
                      widget.draft.kindPayload['hazardsNoted'] = h.toList();
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
