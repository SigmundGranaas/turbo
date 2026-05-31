import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/activity_kind_descriptor.dart';

/// Full-screen container the shell pushes when a marker is tapped (and
/// the kind has opted into the new shell by setting
/// [ActivityKindDescriptor.buildDetailContent]). Owns the app bar, the
/// back affordance, and a thin persistent bottom action bar — the kind
/// only supplies the scrollable body content.
///
/// Old-style detail bottom sheets remain in place for kinds that haven't
/// migrated: the map layer still falls back to [showModalBottomSheet]
/// when [buildDetailContent] is null.
class ActivityDetailScreen extends ConsumerWidget {
  final ActivityKindDescriptor descriptor;
  final String activityId;
  const ActivityDetailScreen({
    super.key,
    required this.descriptor,
    required this.activityId,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final body = descriptor.buildDetailContent!(context, activityId);
    return Scaffold(
      appBar: AppBar(
        backgroundColor: descriptor.tintColor.withValues(alpha: 0.08),
        title: Text(descriptor.displayName),
        elevation: 0,
      ),
      body: SafeArea(
        bottom: false,
        child: SingleChildScrollView(
          padding: const EdgeInsets.only(bottom: 96),
          child: body,
        ),
      ),
    );
  }
}
