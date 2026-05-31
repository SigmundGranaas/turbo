import 'package:flutter/material.dart';

import 'package:turbo/features/activities/api.dart' show ObservationDraft;

const _tintColor = Color(0xFF1E6FB8);

/// Fishing observation extras. The headline is catch / no-catch — over
/// time the orchestrator can learn which conditions correlate with the
/// `caught: true` runs at this spot.
class FishingObservationExtras extends StatefulWidget {
  final ObservationDraft draft;
  final VoidCallback onChanged;

  const FishingObservationExtras({
    super.key,
    required this.draft,
    required this.onChanged,
  });

  @override
  State<FishingObservationExtras> createState() =>
      _FishingObservationExtrasState();
}

class _FishingObservationExtrasState extends State<FishingObservationExtras> {
  static const _clarityOptions = <(String, String)>[
    ('clear', 'Clear'),
    ('stained', 'Stained'),
    ('muddy', 'Muddy / silt'),
  ];

  final _speciesCtrl = TextEditingController();
  final _lengthCtrl = TextEditingController();
  final _weightCtrl = TextEditingController();
  final _lureCtrl = TextEditingController();
  final _concernsCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    _speciesCtrl.text = widget.draft.kindPayload['species'] as String? ?? '';
    _lengthCtrl.text = widget.draft.kindPayload['lengthCm']?.toString() ?? '';
    _weightCtrl.text = widget.draft.kindPayload['weightKg']?.toString() ?? '';
    _lureCtrl.text = widget.draft.kindPayload['lure'] as String? ?? '';
    _concernsCtrl.text = widget.draft.kindPayload['concerns'] as String? ?? '';
  }

  @override
  void dispose() {
    _speciesCtrl.dispose();
    _lengthCtrl.dispose();
    _weightCtrl.dispose();
    _lureCtrl.dispose();
    _concernsCtrl.dispose();
    super.dispose();
  }

  void _setDoubleField(String key, String raw) {
    final n = double.tryParse(raw.replaceAll(',', '.'));
    if (n == null) {
      widget.draft.kindPayload.remove(key);
    } else {
      widget.draft.kindPayload[key] = n;
    }
    widget.onChanged();
  }

  void _setTextField(String key, String raw) {
    final t = raw.trim();
    if (t.isEmpty) {
      widget.draft.kindPayload.remove(key);
    } else {
      widget.draft.kindPayload[key] = t;
    }
    widget.onChanged();
  }

  @override
  Widget build(BuildContext context) {
    final caught = widget.draft.kindPayload['caught'] as bool? ?? false;
    final clarity = widget.draft.kindPayload['waterClarity'] as String?;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: Text(caught ? 'Caught fish' : 'No catch',
              style: const TextStyle(fontWeight: FontWeight.w600)),
          value: caught,
          activeThumbColor: _tintColor,
          onChanged: (v) {
            setState(() => widget.draft.kindPayload['caught'] = v);
            widget.onChanged();
          },
        ),
        if (caught) ...[
          const SizedBox(height: 8),
          TextField(
            controller: _speciesCtrl,
            decoration: const InputDecoration(
              labelText: 'Species', border: OutlineInputBorder()),
            onChanged: (v) => _setTextField('species', v),
          ),
          const SizedBox(height: 8),
          Row(children: [
            Expanded(
              child: TextField(
                controller: _lengthCtrl,
                keyboardType: const TextInputType.numberWithOptions(decimal: true),
                decoration: const InputDecoration(
                  labelText: 'Length (cm)', border: OutlineInputBorder()),
                onChanged: (v) => _setDoubleField('lengthCm', v),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: TextField(
                controller: _weightCtrl,
                keyboardType: const TextInputType.numberWithOptions(decimal: true),
                decoration: const InputDecoration(
                  labelText: 'Weight (kg)', border: OutlineInputBorder()),
                onChanged: (v) => _setDoubleField('weightKg', v),
              ),
            ),
          ]),
          const SizedBox(height: 8),
          TextField(
            controller: _lureCtrl,
            decoration: const InputDecoration(
              labelText: 'Lure / fly', border: OutlineInputBorder()),
            onChanged: (v) => _setTextField('lure', v),
          ),
        ],
        const SizedBox(height: 16),
        Text('Water clarity', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 6),
        Wrap(
          spacing: 8,
          children: [
            for (final (key, label) in _clarityOptions)
              ChoiceChip(
                label: Text(label),
                selected: clarity == key,
                selectedColor: _tintColor.withValues(alpha: 0.20),
                onSelected: (s) {
                  setState(() {
                    if (s) {
                      widget.draft.kindPayload['waterClarity'] = key;
                    } else {
                      widget.draft.kindPayload.remove('waterClarity');
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
          onChanged: (v) => _setTextField('concerns', v),
        ),
      ],
    );
  }
}
