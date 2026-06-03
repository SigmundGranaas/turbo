import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/core/widgets/app_dialog.dart';
import 'package:turbo/core/widgets/app_pill.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/path_recording/api.dart';
import 'package:turbo/features/routing/api.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/settings/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart';

import '../data/active_journey_notifier.dart';
import '../models/active_journey.dart';

/// The single widget for an active "outing" — following a path and/or recording
/// a track. Previously these were two stacked overlays (the recording card and
/// a separate "Following" chip); now one feature owns both states in one
/// widget. Shows the followed-path header (label + distance-to-go + edit/stop)
/// and, when recording, the live stats + pause/save/discard — folding "Record"
/// in when following without capture yet.
class ActiveOutingPanel extends ConsumerStatefulWidget {
  const ActiveOutingPanel({super.key});

  @override
  ConsumerState<ActiveOutingPanel> createState() => _ActiveOutingPanelState();
}

class _ActiveOutingPanelState extends ConsumerState<ActiveOutingPanel> {
  Timer? _ticker;
  bool _arrivalHandled = false;

  static const double _arrivalThresholdM = 30;
  static const double _offRouteThresholdM = 50;

  @override
  void initState() {
    super.initState();
    _ticker = Timer.periodic(const Duration(seconds: 1), (_) {
      if (mounted) setState(() {});
    });
  }

  @override
  void dispose() {
    _ticker?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // Reset the arrival latch whenever an outing ends/changes.
    ref.listen(activeJourneyProvider, (_, next) {
      if (next.kind != JourneyKind.followingPath) _arrivalHandled = false;
    });
    // Auto-finish when within arrival range of the path end. Defer the actual
    // finish out of this notification callback: _handleArrival mutates the
    // journey (stop / stop-and-save), and doing so synchronously here would
    // re-enter the *same* flush that journey-derived providers
    // (NavigationStateNotifier) are already rebuilding in — which Riverpod
    // rejects with "rebuilt multiple times in the same frame". A microtask runs
    // it after the current flush settles.
    ref.listen(journeyProgressProvider, (_, next) {
      if (next != null &&
          next.remainingM < _arrivalThresholdM &&
          !_arrivalHandled) {
        _arrivalHandled = true;
        Future.microtask(() {
          if (mounted) _handleArrival();
        });
      }
    });

    final journey = ref.watch(activeJourneyProvider);
    final rec = ref.watch(recordingNotifierProvider);
    final following = journey.kind == JourneyKind.followingPath;
    final recording = rec.isActive;
    if (!following && !recording) return const SizedBox.shrink();

    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final textTheme = theme.textTheme;
    final unit = ref.watch(settingsProvider
        .select((s) => s.value?.distanceUnit ?? DistanceUnit.metric));

    return AppCardSurface(
      margin: const EdgeInsets.symmetric(horizontal: AppSpacing.l),
      maxWidth: 700,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (following) _followingHeader(journey, scheme, textTheme, unit),
          if (following && recording)
            const Divider(
                height: 1, indent: AppSpacing.l, endIndent: AppSpacing.l),
          if (recording) ..._recordingSection(rec, scheme, textTheme, unit),
          if (following && !recording)
            Padding(
              padding: const EdgeInsets.fromLTRB(
                  AppSpacing.s, 0, AppSpacing.s, AppSpacing.xs),
              child: Row(
                children: [
                  TextButton.icon(
                    onPressed: () =>
                        ref.read(activeJourneyProvider.notifier).startRecording(),
                    icon: const Icon(Icons.fiber_manual_record, size: 18),
                    label: const Text('Record this outing'),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }

  Widget _followingHeader(ActiveJourney journey, ColorScheme scheme,
      TextTheme textTheme, DistanceUnit unit) {
    final progress = ref.watch(journeyProgressProvider);
    final offRoute =
        progress != null && progress.offRouteM > _offRouteThresholdM;
    final subtitle = progress == null
        ? null
        : '${formatDistance(progress.remainingM, unit)} to go · ETA ${_formatEta(progress.etaSeconds)}';

    return Padding(
      padding: const EdgeInsets.fromLTRB(
          AppSpacing.l, AppSpacing.m, AppSpacing.s, AppSpacing.s),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              Icon(Icons.directions_walk, color: scheme.primary),
              const SizedBox(width: AppSpacing.m),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(journey.label ?? 'Following',
                        style: textTheme.titleSmall
                            ?.copyWith(fontWeight: FontWeight.w600),
                        overflow: TextOverflow.ellipsis),
                    if (subtitle != null)
                      Text(subtitle,
                          style: textTheme.bodySmall
                              ?.copyWith(color: scheme.onSurfaceVariant)),
                  ],
                ),
              ),
              if (journey.path != null)
                IconButton(
                  tooltip: 'Download map along route',
                  onPressed: () => _downloadAlongRoute(journey),
                  icon: const Icon(Icons.download_for_offline_outlined),
                ),
              if (journey.waypoints != null)
                IconButton(
                  tooltip: 'Edit route',
                  onPressed: _editRoute,
                  icon: const Icon(Icons.edit_outlined),
                ),
              if (!ref.watch(recordingNotifierProvider).isActive)
                IconButton(
                  tooltip: 'Stop following',
                  onPressed: () =>
                      ref.read(activeJourneyProvider.notifier).stop(),
                  icon: const Icon(Icons.close),
                ),
            ],
          ),
          if (progress != null) ...[
            const SizedBox(height: AppSpacing.s),
            ClipRRect(
              borderRadius: BorderRadius.circular(AppRadius.s),
              child: LinearProgressIndicator(
                value: progress.fraction.clamp(0.0, 1.0),
                minHeight: 6,
                backgroundColor: scheme.surfaceContainerHighest,
              ),
            ),
          ],
          if (offRoute)
            Padding(
              padding: const EdgeInsets.only(top: AppSpacing.s),
              child: Row(
                children: [
                  Icon(Icons.warning_amber_rounded,
                      size: 18, color: scheme.error),
                  const SizedBox(width: AppSpacing.xs),
                  Expanded(
                    child: Text('Off route',
                        style: textTheme.bodySmall
                            ?.copyWith(color: scheme.error)),
                  ),
                  TextButton(onPressed: _reroute, child: const Text('Re-route')),
                ],
              ),
            ),
        ],
      ),
    );
  }

  /// Local arrival clock, e.g. "14:32".
  String _formatEta(double seconds) {
    final t = DateTime.now().add(Duration(seconds: seconds.round()));
    return '${t.hour.toString().padLeft(2, '0')}:${t.minute.toString().padLeft(2, '0')}';
  }

  void _handleArrival() {
    final recording = ref.read(recordingNotifierProvider).isActive;
    if (recording) {
      _stopAndSave(context); // ends the outing + offers to save the track
    } else {
      ref.read(activeJourneyProvider.notifier).stop();
      if (mounted) AppSnackbars.info(context, "You've arrived.");
    }
  }

  /// Re-solve from the user's current position to the destination, keeping any
  /// in-flight recording intact (geometry swap only).
  Future<void> _reroute() async {
    final pos = ref.read(locationStateProvider).value;
    final journey = ref.read(activeJourneyProvider);
    final path = journey.path;
    if (pos == null || path == null || path.points.isEmpty) return;
    final dest = (journey.waypoints != null && journey.waypoints!.isNotEmpty)
        ? journey.waypoints!.last
        : path.points.last;
    try {
      final plan =
          await ref.read(routingRepositoryProvider).plan(points: [pos, dest]);
      ref.read(activeJourneyProvider.notifier).updateFollowedPath(
            plan.toGeoPath(),
            waypoints: [pos, dest],
          );
      _arrivalHandled = false;
    } catch (_) {
      if (mounted) AppSnackbars.error(context, 'Could not re-route');
    }
  }

  List<Widget> _recordingSection(RecordingState rec, ColorScheme scheme,
      TextTheme textTheme, DistanceUnit unit) {
    final isPaused = rec.status == RecordingStatus.paused;
    final accent = isPaused ? scheme.tertiary : scheme.error;
    final canSave = rec.points.length >= 2;
    final duration = rec.startedAt == null
        ? Duration.zero
        : DateTime.now().difference(rec.startedAt!);

    return [
      Padding(
        padding: const EdgeInsets.fromLTRB(
            AppSpacing.l, AppSpacing.m, AppSpacing.l, AppSpacing.m),
        child: Row(
          children: [
            _StatusDot(color: accent, paused: isPaused),
            const SizedBox(width: AppSpacing.m),
            Expanded(
              child: Row(
                children: [
                  Expanded(
                      child: _Stat('Distance',
                          formatDistance(rec.distanceMeters, unit), textTheme, scheme)),
                  Expanded(
                      child: _Stat('Time', _fmtDuration(duration), textTheme, scheme)),
                  Expanded(
                      child: _Stat('Elev gain', _fmtElev(rec.ascent, unit),
                          textTheme, scheme)),
                ],
              ),
            ),
            const SizedBox(width: AppSpacing.s),
            AppButton.tonal(
              text: 'Save',
              onPressed: canSave ? () => _stopAndSave(context) : null,
            ),
          ],
        ),
      ),
      const Divider(height: 1, indent: AppSpacing.l, endIndent: AppSpacing.l),
      Padding(
        padding: const EdgeInsets.symmetric(
            horizontal: AppSpacing.m, vertical: AppSpacing.xs),
        child: Row(
          children: [
            IconButton(
              onPressed: isPaused
                  ? ref.read(recordingNotifierProvider.notifier).resume
                  : ref.read(recordingNotifierProvider.notifier).pause,
              icon: Icon(isPaused ? Icons.play_arrow : Icons.pause),
              tooltip: isPaused ? 'Resume' : 'Pause',
            ),
            const Spacer(),
            IconButton(
              onPressed: () => _confirmDiscard(context),
              icon: const Icon(Icons.delete_sweep_outlined),
              tooltip: 'Discard recording',
            ),
          ],
        ),
      ),
    ];
  }

  void _editRoute() {
    final wps = ref.read(activeJourneyProvider).waypoints;
    if (wps == null || wps.isEmpty) return;
    final notifier = ref.read(routePlanningProvider.notifier);
    notifier.clear();
    for (final w in wps) {
      notifier.addWaypoint(w);
    }
    ref.read(activeJourneyProvider.notifier).stop();
    ref.read(activeMapToolProvider.notifier).activate(routePlanningToolId);
  }

  /// Download the map tiles along the followed route — buffers the path into a
  /// corridor and opens the standard download sheet. Lets the user grab offline
  /// coverage for the exact route they're about to walk.
  void _downloadAlongRoute(ActiveJourney journey) {
    final path = journey.path;
    if (path == null || path.isEmpty) return;
    showExclusiveSheet<void>(
      context,
      builder: (_) => DownloadDetailsSheet(bounds: corridorBounds(path)),
    );
  }

  /// Ends the whole outing: stop+save the recording, then stop following.
  Future<void> _stopAndSave(BuildContext context) async {
    final messenger = ScaffoldMessenger.maybeOf(context);
    final journey = ref.read(activeJourneyProvider);
    final journeyNotifier = ref.read(activeJourneyProvider.notifier);
    final wasFollowing = journey.isActive;
    // Capture the planned route (when following one) so the saved track can show
    // planned-vs-actual. Only meaningful when there were real waypoints.
    final planned = (journey.kind == JourneyKind.followingPath &&
            journey.waypoints != null &&
            journey.waypoints!.isNotEmpty)
        ? journey.path?.points
        : null;
    final result = await ref.read(recordingNotifierProvider.notifier).stop();
    if (result == null || result.isEmpty) {
      messenger?.showSnackBar(const SnackBar(
          content: Text('No track to save — not enough fixes.')));
      if (wasFollowing) journeyNotifier.stop();
      return;
    }
    if (!context.mounted) return;
    await showExclusiveSheet<bool>(
      context,
      builder: (_) => SavePathSheet.fromGeoPath(
        result.toGeoPath(),
        plannedGeometry: planned,
      ),
    );
    if (wasFollowing) journeyNotifier.stop();
  }

  Future<void> _confirmDiscard(BuildContext context) async {
    final confirmed = await AppDialog.destructive(
      context,
      title: 'Discard recording?',
      content: 'This will throw away the track you just recorded.',
      destructiveLabel: 'Discard',
    );
    if (!confirmed) return;
    await ref.read(recordingNotifierProvider.notifier).discard();
    if (ref.read(activeJourneyProvider).isActive) {
      ref.read(activeJourneyProvider.notifier).stop();
    }
    if (context.mounted) AppSnackbars.info(context, 'Recording discarded.');
  }

  String _fmtDuration(Duration d) {
    final h = d.inHours;
    final m = d.inMinutes.remainder(60).toString().padLeft(2, '0');
    final s = d.inSeconds.remainder(60).toString().padLeft(2, '0');
    return h > 0 ? '$h:$m:$s' : '$m:$s';
  }

  String _fmtElev(double meters, DistanceUnit unit) =>
      unit == DistanceUnit.imperial
          ? '${(meters / 0.3048).round()} ft'
          : '${meters.round()} m';
}

class _Stat extends StatelessWidget {
  final String label;
  final String value;
  final TextTheme textTheme;
  final ColorScheme scheme;
  const _Stat(this.label, this.value, this.textTheme, this.scheme);

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(label,
            style: textTheme.bodySmall
                ?.copyWith(color: scheme.onSurfaceVariant)),
        Text(value,
            style: textTheme.titleMedium?.copyWith(
                fontWeight: FontWeight.bold, color: scheme.onSurface)),
      ],
    );
  }
}

class _StatusDot extends StatefulWidget {
  final Color color;
  final bool paused;
  const _StatusDot({required this.color, required this.paused});

  @override
  State<_StatusDot> createState() => _StatusDotState();
}

class _StatusDotState extends State<_StatusDot>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      duration: const Duration(milliseconds: 1100),
      vsync: this,
    )..repeat(reverse: true);
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (widget.paused) {
      return Icon(Icons.pause_circle_filled, color: widget.color, size: 22);
    }
    return AnimatedBuilder(
      animation: _controller,
      builder: (_, _) => Container(
        width: 14,
        height: 14,
        decoration: BoxDecoration(
          shape: BoxShape.circle,
          color: widget.color.withValues(alpha: 0.5 + 0.5 * _controller.value),
        ),
      ),
    );
  }
}
