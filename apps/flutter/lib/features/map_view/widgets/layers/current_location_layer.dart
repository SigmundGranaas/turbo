import 'dart:io';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:turbo/core/widgets/map/map_line_style.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';
import 'package:path/path.dart' as p;

import 'package:turbo/core/location/compass_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/app/location_marker_tokens.dart';
import 'package:turbo/features/markers/api.dart' hide Marker;
import 'package:turbo/features/path_recording/api.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/settings/api.dart';

class CurrentLocationLayer extends ConsumerWidget {
  const CurrentLocationLayer({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final locationState = ref.watch(locationStateProvider);
    final settingsAsync = ref.watch(settingsProvider);

    return locationState.when(
      data: (location) {
        if (location == null) return const SizedBox.shrink();

        final settings = settingsAsync.value;
        final scale = settings?.locationMarkerSize ?? 1.0;
        final showHeading = settings?.showHeadingArrow ?? false;
        final arrowColor = hexToColor(settings?.markerArrowColorHex) ??
            LocationMarkerTokens.defaultFill;
        final outlineColor = hexToColor(settings?.markerOutlineColorHex) ??
            LocationMarkerTokens.defaultOutline;

        // Base marker size is 40; heading arrow adds extra space.
        final baseSize = 40.0 * scale;
        final markerSize = showHeading ? baseSize * 2.0 : baseSize + 20;

        // Only watch compass when heading arrow is enabled.
        final double? heading;
        if (showHeading) {
          heading = ref.watch(compassStateProvider).value;
        } else {
          heading = null;
        }

        final isRecording =
            ref.watch(recordingNotifierProvider).isActive;
        return MarkerLayer(
          markers: [
            Marker(
              width: markerSize,
              height: markerSize,
              point: location,
              child: GestureDetector(
                onTap: () => showLocationIconPickerSheet(context, ref),
                // Long-press on the marker is the discoverable way to start
                // a recording — keeps the chrome uncluttered and contextual.
                // If a recording is already in flight, the bottom panel is
                // the control surface; long-press is a no-op there.
                onLongPress: isRecording
                    ? null
                    : () => startRecordingFlow(context, ref),
                child: CurrentLocationMarker(
                  iconType: settings?.locationIconType ?? 'default',
                  iconKey: settings?.locationIconKey,
                  imagePath: settings?.locationImagePath,
                  scale: scale,
                  showHeading: showHeading,
                  headingDegrees: heading,
                  arrowColor: arrowColor,
                  outlineColor: outlineColor,
                  isRecording: isRecording,
                ),
              ),
            ),
          ],
        );
      },
      loading: () => const SizedBox.shrink(),
      error: (error, stack) => const SizedBox.shrink(),
    );
  }
}

class CurrentLocationMarker extends StatelessWidget {
  final String iconType;
  final String? iconKey;
  final String? imagePath;
  final double scale;
  final bool showHeading;
  final double? headingDegrees;
  final Color arrowColor;
  final Color outlineColor;
  final bool isRecording;

  const CurrentLocationMarker({
    super.key,
    required this.iconType,
    this.iconKey,
    this.imagePath,
    this.scale = 1.0,
    this.showHeading = false,
    this.headingDegrees,
    this.arrowColor = LocationMarkerTokens.defaultFill,
    this.outlineColor = LocationMarkerTokens.defaultOutline,
    this.isRecording = false,
  });

  @override
  Widget build(BuildContext context) {
    final dotSize = 16.0 * scale;
    final haloSize = 32.0 * scale;

    Widget iconWidget = _buildIcon(context, dotSize, haloSize);

    if (isRecording) {
      iconWidget = _RecordingHalo(
        baseSize: haloSize,
        // Shared recording red (matches the recording trace line) — NOT theme
        // `error`, a pale salmon in dark mode that barely reads on the topo.
        color: MapLineStyle.recording,
        child: iconWidget,
      );
    }

    if (!showHeading || headingDegrees == null) {
      return iconWidget;
    }

    final headingRadians = headingDegrees! * (math.pi / 180.0);
    return Stack(
      alignment: Alignment.center,
      children: [
        // Heading arrow behind the icon.
        Transform.rotate(
          angle: headingRadians,
          child: CustomPaint(
            size: Size(haloSize * 2, haloSize * 2),
            painter: HeadingIndicatorPainter(
              color: arrowColor.withValues(alpha: 0.4),
            ),
          ),
        ),
        iconWidget,
      ],
    );
  }

  Widget _buildIcon(BuildContext context, double dotSize, double haloSize) {
    switch (iconType) {
      case 'builtin':
        return _buildBuiltinIcon(context, dotSize, haloSize);
      case 'custom':
        return _buildCustomIcon(context, dotSize, haloSize);
      default:
        return _buildDefaultDot(dotSize, haloSize);
    }
  }

  Widget _buildDefaultDot(double dotSize, double haloSize) {
    return Stack(
      alignment: Alignment.center,
      children: [
        Container(
          width: haloSize,
          height: haloSize,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: LocationMarkerTokens.defaultFill.withValues(alpha: 0.3),
          ),
        ),
        Container(
          width: dotSize,
          height: dotSize,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: LocationMarkerTokens.defaultFill,
            border: Border.all(color: outlineColor, width: 2),
          ),
        ),
      ],
    );
  }

  Widget _buildBuiltinIcon(
      BuildContext context, double dotSize, double haloSize) {
    final iconService = IconService();
    final namedIcon = iconService.getIcon(context, iconKey);
    return Stack(
      alignment: Alignment.center,
      children: [
        Container(
          width: haloSize,
          height: haloSize,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: LocationMarkerTokens.defaultFill.withValues(alpha: 0.2),
          ),
        ),
        Container(
          width: dotSize + 8 * scale,
          height: dotSize + 8 * scale,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: LocationMarkerTokens.defaultFill,
            border: Border.all(color: outlineColor, width: 2),
          ),
          child: Icon(
            namedIcon.icon,
            size: dotSize * 0.75,
            color: LocationMarkerTokens.defaultOutline,
          ),
        ),
      ],
    );
  }

  Widget _buildCustomIcon(
      BuildContext context, double dotSize, double haloSize) {
    if (imagePath == null) return _buildDefaultDot(dotSize, haloSize);

    return FutureBuilder<Directory>(
      future: getApplicationDocumentsDirectory(),
      builder: (context, snapshot) {
        if (!snapshot.hasData) return _buildDefaultDot(dotSize, haloSize);

        final fullPath = p.join(snapshot.data!.path, imagePath!);
        final file = File(fullPath);
        if (!file.existsSync()) return _buildDefaultDot(dotSize, haloSize);

        final imageSize = dotSize + 8 * scale;
        return Stack(
          alignment: Alignment.center,
          children: [
            Container(
              width: haloSize,
              height: haloSize,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: LocationMarkerTokens.defaultFill.withValues(alpha: 0.2),
              ),
            ),
            Container(
              width: imageSize,
              height: imageSize,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                border: Border.all(color: outlineColor, width: 2),
              ),
              child: ClipOval(
                child: Image.file(file, fit: BoxFit.cover),
              ),
            ),
          ],
        );
      },
    );
  }
}

/// Paints a directional heading indicator — a sleek tapered point extending
/// upward from center, similar to the DropletPainter pin shape but inverted.
class HeadingIndicatorPainter extends CustomPainter {
  final Color color;

  HeadingIndicatorPainter({required this.color});

  @override
  void paint(Canvas canvas, Size size) {
    final double w = size.width;
    final double h = size.height;
    final double cx = w / 2;
    final double cy = h / 2;

    // The arrow points upward from center. The tip is at the top.
    final tipY = cy * 0.15; // Tip near the top.
    final baseRadius = w * 0.15; // Width at the base (near center).

    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.fill;

    final path = Path()
      ..moveTo(cx, tipY)
      ..cubicTo(
        cx, cy * 0.55,
        cx - baseRadius * 1.8, cy * 0.7,
        cx - baseRadius, cy,
      )
      ..arcTo(
        Rect.fromCircle(center: Offset(cx, cy), radius: baseRadius),
        math.pi,
        -math.pi,
        false,
      )
      ..cubicTo(
        cx + baseRadius * 1.8, cy * 0.7,
        cx, cy * 0.55,
        cx, tipY,
      )
      ..close();

    canvas.drawPath(path, paint);
  }

  @override
  bool shouldRepaint(covariant HeadingIndicatorPainter oldDelegate) {
    return oldDelegate.color != color;
  }
}

/// Pulsing colored ring drawn behind the location marker while a recording
/// is active. Surfaces "we are recording" visually without adding chrome.
class _RecordingHalo extends StatefulWidget {
  final double baseSize;
  final Color color;
  final Widget child;

  const _RecordingHalo({
    required this.baseSize,
    required this.color,
    required this.child,
  });

  @override
  State<_RecordingHalo> createState() => _RecordingHaloState();
}

class _RecordingHaloState extends State<_RecordingHalo>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      duration: const Duration(milliseconds: 1400),
      vsync: this,
    )..repeat();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Stack(
      alignment: Alignment.center,
      children: [
        AnimatedBuilder(
          animation: _controller,
          builder: (_, _) {
            final t = _controller.value;
            // Halo grows from 1.0x → 1.8x base and fades out as it expands.
            final size = widget.baseSize * (1.0 + t * 0.8);
            final opacity = (1.0 - t).clamp(0.0, 1.0) * 0.45;
            return Container(
              width: size,
              height: size,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: widget.color.withValues(alpha: opacity),
              ),
            );
          },
        ),
        widget.child,
      ],
    );
  }
}
