import 'package:flutter/material.dart';

/// Shared "Delete X?" confirm dialog used by every per-kind detail
/// sheet. Returns `true` when the user confirms, `false`/`null`
/// otherwise — callers should treat anything but `true` as cancel.
Future<bool> showActivityDeleteDialog(
  BuildContext context, {
  required String name,
  required String kindNoun,
}) async {
  final result = await showDialog<bool>(
    context: context,
    builder: (ctx) => AlertDialog(
      title: Text('Delete $kindNoun?'),
      content: Text('"$name" will be removed.'),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(false),
          child: const Text('Cancel'),
        ),
        FilledButton(
          style: FilledButton.styleFrom(
            backgroundColor: Theme.of(ctx).colorScheme.error,
          ),
          onPressed: () => Navigator.of(ctx).pop(true),
          child: const Text('Delete'),
        ),
      ],
    ),
  );
  return result == true;
}
