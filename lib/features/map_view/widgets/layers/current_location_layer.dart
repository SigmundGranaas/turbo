import 'dart:io';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';
import 'package:path/path.dart' as p;

import 'package:turbo/core/location/compass_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/features/markers/data/icon_service.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';
import 'package:turbo/features/settings/widgets/location_icon_picker_sheet.dart';

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

        return MarkerLayer(
          markers: [
            Marker(
              width: markerSize,
              height: markerSize,
              point: location,
              child: GestureDetector(
                onTap: () => showLocationIconPickerSheet(context, ref),
                child: CurrentLocationMarker(
                  iconType: settings?.locationIconType ?? 'default',
                  iconKey: settings?.locationIconKey,
                  imagePath: settings?.locationImagePath,
                  scale: scale,
                  showHeading: showHeading,
                  headingDegrees: heading,
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

  const CurrentLocationMarker({
    super.key,
    required this.iconType,
    this.iconKey,
    this.imagePath,
    this.scale = 1.0,
    this.showHeading = false,
    this.headingDegrees,
  });

  @override
  Widget build(BuildContext context) {
    final dotSize = 16.0 * scale;
    final haloSize = 32.0 * scale;

    final iconWidget = _buildIcon(context, dotSize, haloSize);

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
              color: Theme.of(context).colorScheme.primary.withValues(alpha: 0.4),
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
            color: Colors.lightBlue.withValues(alpha: 0.3),
          ),
        ),
        Container(
          width: dotSize,
          height: dotSize,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: Colors.lightBlue,
            border: Border.all(color: Colors.white, width: 2),
          ),
        ),
      ],
    );
  }

  Widget _buildBuiltinIcon(
      BuildContext context, double dotSize, double haloSize) {
    final colorScheme = Theme.of(context).colorScheme;
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
            color: colorScheme.primary.withValues(alpha: 0.2),
          ),
        ),
        Container(
          width: dotSize + 8 * scale,
          height: dotSize + 8 * scale,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: colorScheme.primary,
            border: Border.all(color: Colors.white, width: 2),
          ),
          child: Icon(
            namedIcon.icon,
            size: dotSize * 0.75,
            color: Colors.white,
          ),
        ),
      ],
    );
  }

  Widget _buildCustomIcon(
      BuildContext context, double dotSize, double haloSize) {
    final colorScheme = Theme.of(context).colorScheme;
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
                color: colorScheme.primary.withValues(alpha: 0.2),
              ),
            ),
            Container(
              width: imageSize,
              height: imageSize,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                border: Border.all(color: Colors.white, width: 2),
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
