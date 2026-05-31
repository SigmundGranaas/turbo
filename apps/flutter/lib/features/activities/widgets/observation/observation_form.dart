import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';

import 'package:turbo/features/auth/api.dart';

import 'observation_draft.dart';

final _log = Logger('ObservationForm');

/// Submit handler signature passed to [ActivityObservationForm]. Returns
/// once the POST has succeeded so the form can pop with a success result
/// and the caller can invalidate the relevant analysis provider.
typedef ObservationSubmit = Future<void> Function({
  required DateTime observedAt,
  required int? rating,
  required String? comment,
  required int photoCount,
  required Map<String, Object?> kindPayload,
});

/// Shared post-visit form. Owns the always-required fields (date, rating,
/// comment, photo count); per-kind extras plug in via the [extrasBuilder]
/// callback which receives the live [ObservationDraft] and renders a list
/// of [Widget]s slotted between the rating and the comment.
///
/// The form does not own the HTTP call — callers pass [onSubmit] which
/// handles the per-kind endpoint, headers, and any post-submit cache
/// invalidation. This keeps the kind-specific URL slug + payload-shape
/// concerns out of the shell.
class ActivityObservationForm extends ConsumerStatefulWidget {
  final String kindKey;
  final String activityName;
  final Color tintColor;
  final List<Widget> Function(BuildContext context, ObservationDraft draft, void Function() onChanged)
      extrasBuilder;
  final ObservationSubmit onSubmit;

  const ActivityObservationForm({
    super.key,
    required this.kindKey,
    required this.activityName,
    required this.tintColor,
    required this.extrasBuilder,
    required this.onSubmit,
  });

  @override
  ConsumerState<ActivityObservationForm> createState() => _ActivityObservationFormState();
}

class _ActivityObservationFormState extends ConsumerState<ActivityObservationForm> {
  late ObservationDraft _draft;
  final _commentCtrl = TextEditingController();
  bool _submitting = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _draft = ObservationDraft(observedAt: DateTime.now());
  }

  @override
  void dispose() {
    _commentCtrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(
        title: Text('Log visit · ${widget.activityName}'),
        backgroundColor: widget.tintColor.withValues(alpha: 0.08),
      ),
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _DateRow(
                value: _draft.observedAt,
                onChanged: (d) => setState(() => _draft.observedAt = d),
              ),
              const SizedBox(height: 20),
              Text('How was it?', style: theme.textTheme.titleMedium),
              const SizedBox(height: 4),
              _RatingRow(
                value: _draft.rating,
                tintColor: widget.tintColor,
                onChanged: (r) => setState(() => _draft.rating = r),
              ),
              const SizedBox(height: 24),
              ...widget.extrasBuilder(context, _draft, () => setState(() {})),
              const SizedBox(height: 24),
              Text('Notes', style: theme.textTheme.titleMedium),
              const SizedBox(height: 6),
              TextField(
                controller: _commentCtrl,
                minLines: 3,
                maxLines: 5,
                decoration: const InputDecoration(
                  hintText: 'Anything worth telling the future you?',
                  border: OutlineInputBorder(),
                ),
              ),
              if (_error != null) ...[
                const SizedBox(height: 12),
                Text(_error!,
                    style: TextStyle(color: theme.colorScheme.error)),
              ],
              const SizedBox(height: 16),
              FilledButton.icon(
                onPressed: _submitting ? null : _submit,
                icon: _submitting
                    ? const SizedBox(
                        width: 16, height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.check_rounded),
                label: Text(_submitting ? 'Saving…' : 'Save'),
                style: FilledButton.styleFrom(
                  backgroundColor: widget.tintColor,
                  padding: const EdgeInsets.symmetric(vertical: 14),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _submit() async {
    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      _draft.comment = _commentCtrl.text.trim().isEmpty ? null : _commentCtrl.text.trim();
      await widget.onSubmit(
        observedAt: _draft.observedAt,
        rating: _draft.rating,
        comment: _draft.comment,
        photoCount: _draft.photoCount,
        kindPayload: _draft.kindPayload,
      );
      if (!mounted) return;
      Navigator.of(context).pop(true);
    } catch (e, st) {
      _log.warning('Observation save failed', e, st);
      if (!mounted) return;
      setState(() {
        _submitting = false;
        _error = 'Could not save: $e';
      });
    }
  }
}

class _DateRow extends StatelessWidget {
  final DateTime value;
  final ValueChanged<DateTime> onChanged;
  const _DateRow({required this.value, required this.onChanged});

  @override
  Widget build(BuildContext context) {
    final f = MaterialLocalizations.of(context);
    return Row(
      children: [
        const Icon(Icons.event_outlined),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            'Visited ${f.formatMediumDate(value)}',
            style: Theme.of(context).textTheme.titleMedium,
          ),
        ),
        TextButton(
          onPressed: () async {
            final picked = await showDatePicker(
              context: context,
              initialDate: value,
              firstDate: DateTime.now().subtract(const Duration(days: 365)),
              lastDate: DateTime.now(),
            );
            if (picked != null) onChanged(picked);
          },
          child: const Text('Change'),
        ),
      ],
    );
  }
}

class _RatingRow extends StatelessWidget {
  final int? value;
  final Color tintColor;
  final ValueChanged<int?> onChanged;
  const _RatingRow({required this.value, required this.tintColor, required this.onChanged});

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        for (var i = 1; i <= 5; i++)
          IconButton(
            visualDensity: VisualDensity.compact,
            onPressed: () => onChanged(value == i ? null : i),
            icon: Icon(
              (value ?? 0) >= i ? Icons.star_rounded : Icons.star_outline_rounded,
              size: 30,
              color: (value ?? 0) >= i ? tintColor : Colors.grey,
            ),
          ),
        const Spacer(),
        if (value != null)
          Text('$value / 5', style: TextStyle(color: tintColor, fontWeight: FontWeight.w600)),
      ],
    );
  }
}

/// HTTP helper that wraps the boilerplate around the per-kind POST. Each
/// kind's "Log visit" button passes its URL slug + payload mapper here.
/// HTTP helper that wraps the boilerplate around the per-kind POST.
/// Takes a [WidgetRef] because the caller is always a widget callback —
/// not a provider — and Riverpod's [Ref] and [WidgetRef] are distinct
/// types.
Future<void> postObservation({
  required WidgetRef ref,
  required String kindUrlSlug,
  required String activityId,
  required DateTime observedAt,
  required int? rating,
  required String? comment,
  required int photoCount,
  required Map<String, Object?> kindPayload,
}) async {
  final client = ref.read(authenticatedApiClientProvider);
  final body = <String, Object?>{
    'observedAt': observedAt.toUtc().toIso8601String(),
    'rating': ?rating,
    if (comment != null && comment.isNotEmpty) 'comment': comment,
    if (photoCount > 0) 'photoCount': photoCount,
    ...kindPayload,
  };
  final r = await client.post(
    '/api/activities/$kindUrlSlug/$activityId/observations',
    data: body,
  );
  if (r.statusCode != 200 && r.statusCode != 201) {
    throw Exception('Observation save failed: HTTP ${r.statusCode}');
  }
}
