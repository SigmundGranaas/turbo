import 'package:flutter/material.dart';

import 'package:turbo/features/activities/api.dart' show ObservationDraft;

const _tintColor = Color(0xFF5E72A5);

/// Backcountry ski observation extras — the safety-critical kind.
/// Surfaces snow quality, signs of instability, the user's observed
/// danger level (calibrates against Varsom's bulletin over time).
class BackcountrySkiObservationExtras extends StatefulWidget {
  final ObservationDraft draft;
  final VoidCallback onChanged;

  const BackcountrySkiObservationExtras({
    super.key,
    required this.draft,
    required this.onChanged,
  });

  @override
  State<BackcountrySkiObservationExtras> createState() =>
      _BackcountrySkiObservationExtrasState();
}

class _BackcountrySkiObservationExtrasState
    extends State<BackcountrySkiObservationExtras> {
  static const _snowOptions = <(String, String)>[
    ('powder', 'Powder'),
    ('hard_pack', 'Hard pack'),
    ('breakable_crust', 'Breakable crust'),
    ('wind_slab_present', 'Wind slab present'),
    ('corn', 'Corn'),
    ('wet', 'Wet'),
  ];

  static const _signOptions = <(String, String)>[
    ('recent_avalanche', 'Recent avalanche'),
    ('whoomphing', 'Whoomphing'),
    ('shooting_cracks', 'Shooting cracks'),
    ('hollow_drum', 'Hollow drum sound'),
  ];

  final _concernsCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    _concernsCtrl.text = widget.draft.kindPayload['concerns'] as String? ?? '';
  }

  @override
  void dispose() {
    _concernsCtrl.dispose();
    super.dispose();
  }

  Set<String> _signs() {
    final raw = widget.draft.kindPayload['signsOfInstability'];
    if (raw is List) return raw.cast<String>().toSet();
    return {};
  }

  void _toggleSign(String code, bool on) {
    final s = _signs();
    if (on) {
      s.add(code);
    } else {
      s.remove(code);
    }
    if (s.isEmpty) {
      widget.draft.kindPayload.remove('signsOfInstability');
    } else {
      widget.draft.kindPayload['signsOfInstability'] = s.toList();
    }
  }

  @override
  Widget build(BuildContext context) {
    final snow = widget.draft.kindPayload['snowConditionSummary'] as String?;
    final breakable = widget.draft.kindPayload['breakableCrust'] as bool?;
    final observed = widget.draft.kindPayload['observedDangerLevel'] as int?;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Snow condition', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 6),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            for (final (key, label) in _snowOptions)
              ChoiceChip(
                label: Text(label),
                selected: snow == key,
                selectedColor: _tintColor.withValues(alpha: 0.20),
                onSelected: (s) {
                  setState(() {
                    if (s) {
                      widget.draft.kindPayload['snowConditionSummary'] = key;
                    } else {
                      widget.draft.kindPayload.remove('snowConditionSummary');
                    }
                  });
                  widget.onChanged();
                },
              ),
          ],
        ),
        const SizedBox(height: 16),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: const Text('Breakable crust encountered'),
          value: breakable ?? false,
          activeThumbColor: _tintColor,
          onChanged: (b) {
            setState(() => widget.draft.kindPayload['breakableCrust'] = b);
            widget.onChanged();
          },
        ),
        const SizedBox(height: 16),
        Text('Signs of instability',
            style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 6),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            for (final (key, label) in _signOptions)
              FilterChip(
                label: Text(label),
                selected: _signs().contains(key),
                selectedColor: Colors.red.shade400.withValues(alpha: 0.25),
                onSelected: (s) {
                  setState(() => _toggleSign(key, s));
                  widget.onChanged();
                },
              ),
          ],
        ),
        const SizedBox(height: 16),
        Text('Observed danger level (1–5)',
            style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 4),
        Text('How dangerous did it actually feel, compared to today\'s Varsom bulletin?',
            style: Theme.of(context).textTheme.bodySmall),
        const SizedBox(height: 6),
        Wrap(
          spacing: 6,
          children: [
            for (var i = 1; i <= 5; i++)
              ChoiceChip(
                label: Text(i.toString()),
                selected: observed == i,
                selectedColor: _tintColor.withValues(alpha: 0.20),
                onSelected: (s) {
                  setState(() {
                    if (s) {
                      widget.draft.kindPayload['observedDangerLevel'] = i;
                    } else {
                      widget.draft.kindPayload.remove('observedDangerLevel');
                    }
                  });
                  widget.onChanged();
                },
              ),
          ],
        ),
        const SizedBox(height: 16),
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
