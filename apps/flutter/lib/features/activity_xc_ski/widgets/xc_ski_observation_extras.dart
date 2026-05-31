import 'package:flutter/material.dart';

import 'package:turbo/features/activities/api.dart' show ObservationDraft;

const _tintColor = Color(0xFF0288D1);

/// XC-ski-specific extras for the shared observation form. Lets users
/// report what they actually skied — track condition, snow quality,
/// whether grooming was visible, anything concerning. The whole row
/// writes back into [draft.kindPayload] using the wire-shape keys the
/// XcSki API expects (`trackCondition`, `snowQuality`, etc.).
class XcSkiObservationExtras extends StatefulWidget {
  final ObservationDraft draft;
  final VoidCallback onChanged;

  const XcSkiObservationExtras({
    super.key,
    required this.draft,
    required this.onChanged,
  });

  @override
  State<XcSkiObservationExtras> createState() => _XcSkiObservationExtrasState();
}

class _XcSkiObservationExtrasState extends State<XcSkiObservationExtras> {
  static const _trackOptions = <(String, String)>[
    ('fast_glide', 'Fast glide'),
    ('frozen_granular', 'Frozen granular'),
    ('sticky', 'Sticky'),
    ('icy_ruts', 'Icy ruts'),
    ('deep_unbroken', 'Deep, unbroken'),
    ('wet', 'Wet / slushy'),
  ];

  static const _snowOptions = <(String, String)>[
    ('powder', 'Powder'),
    ('hard_pack', 'Hard pack'),
    ('breakable_crust', 'Breakable crust'),
    ('wet', 'Wet'),
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

  String? _currentTrack() => widget.draft.kindPayload['trackCondition'] as String?;
  String? _currentSnow() => widget.draft.kindPayload['snowQuality'] as String?;
  bool? _currentFreshGrooming() => widget.draft.kindPayload['freshGroomingVisible'] as bool?;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Track condition', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 6),
        _ChipGroup(
          options: _trackOptions,
          value: _currentTrack(),
          onChanged: (v) {
            setState(() {
              if (v == null) {
                widget.draft.kindPayload.remove('trackCondition');
              } else {
                widget.draft.kindPayload['trackCondition'] = v;
              }
            });
            widget.onChanged();
          },
        ),
        const SizedBox(height: 20),
        Text('Snow quality', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 6),
        _ChipGroup(
          options: _snowOptions,
          value: _currentSnow(),
          onChanged: (v) {
            setState(() {
              if (v == null) {
                widget.draft.kindPayload.remove('snowQuality');
              } else {
                widget.draft.kindPayload['snowQuality'] = v;
              }
            });
            widget.onChanged();
          },
        ),
        const SizedBox(height: 16),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          dense: true,
          title: const Text('Fresh grooming visible'),
          value: _currentFreshGrooming() ?? false,
          activeThumbColor: _tintColor,
          onChanged: (b) {
            setState(() => widget.draft.kindPayload['freshGroomingVisible'] = b);
            widget.onChanged();
          },
        ),
        const SizedBox(height: 12),
        Text('Concerns (optional)', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 4),
        TextField(
          controller: _concernsCtrl,
          decoration: const InputDecoration(
            hintText: 'e.g. icy descent on the back loop',
            border: OutlineInputBorder(),
          ),
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

class _ChipGroup extends StatelessWidget {
  final List<(String, String)> options;
  final String? value;
  final ValueChanged<String?> onChanged;

  const _ChipGroup({required this.options, required this.value, required this.onChanged});

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        for (final (key, label) in options)
          ChoiceChip(
            label: Text(label),
            selected: value == key,
            selectedColor: _tintColor.withValues(alpha: 0.20),
            onSelected: (s) => onChanged(s ? key : null),
          ),
      ],
    );
  }
}
