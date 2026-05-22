import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import '../../../core/sharing/shareable_link_codec.dart';
import '../data/pending_share_provider.dart';
import 'shared_marker_preview_sheet.dart';
import 'shared_path_preview_sheet.dart';

/// Watches [pendingShareProvider]; when a payload arrives, opens the
/// matching preview sheet over the map. Designed to wrap the map page.
class SharedPayloadListener extends ConsumerStatefulWidget {
  final Widget child;

  /// Optional hook the host (the map page) can use to recenter the map on
  /// the shared item before the preview sheet appears.
  final void Function(LatLng center)? onCenter;

  const SharedPayloadListener({
    super.key,
    required this.child,
    this.onCenter,
  });

  @override
  ConsumerState<SharedPayloadListener> createState() =>
      _SharedPayloadListenerState();
}

class _SharedPayloadListenerState
    extends ConsumerState<SharedPayloadListener> {
  @override
  void initState() {
    super.initState();
    // Drain any payload that arrived before the listener mounted (cold-start).
    WidgetsBinding.instance.addPostFrameCallback((_) => _drain());
  }

  void _drain() {
    final payload =
        ref.read(pendingShareProvider.notifier).consume();
    if (payload != null) _openSheet(payload);
  }

  void _openSheet(SharedPayload payload) {
    if (!mounted) return;
    final center = switch (payload) {
      SharedMarkerPayload(:final marker) => marker.position,
      SharedPathPayload(:final path) =>
        path.points.isEmpty ? null : path.bounds.center,
    };
    if (center != null) widget.onCenter?.call(center);

    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (_) => switch (payload) {
        SharedMarkerPayload(:final marker) =>
          SharedMarkerPreviewSheet(marker: marker),
        SharedPathPayload(:final path) =>
          SharedPathPreviewSheet(path: path),
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    // Subsequent payloads (mobile deep links while app is open).
    ref.listen<SharedPayload?>(pendingShareProvider, (prev, next) {
      if (next != null) {
        // consume to clear the state before opening so re-listens don't
        // re-trigger.
        final taken = ref.read(pendingShareProvider.notifier).consume();
        if (taken != null) _openSheet(taken);
      }
    });

    return widget.child;
  }
}
