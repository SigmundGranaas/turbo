import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_svg/flutter_svg.dart';

class CustomMapCompass extends StatefulWidget {
  final MapController mapController;
  final String svgAssetPath = 'svg/compass.svg';

  const CustomMapCompass({
    super.key,
    required this.mapController,
  });

  @override
  State<CustomMapCompass> createState() => _CustomMapCompassState();
}

class _CustomMapCompassState extends State<CustomMapCompass> with TickerProviderStateMixin {
  double _rotation = 0.0;

  @override
  void initState() {
    super.initState();
    widget.mapController.mapEventStream.listen((event) {
      if (event is MapEventRotate) {
        setState(() {
          _rotation = event.camera.rotation;
        });
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: _resetRotation,
      child: Card(
        elevation: 4,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
        child: Container(
          padding: const EdgeInsets.all(8),
          child: Transform.rotate(
            angle: -_rotation * (pi / 180),
            child: SvgPicture.asset(
              widget.svgAssetPath,
              width: 40,
              height: 40,
            ),
          ),
        ),
      ),
    );
  }

  void _resetRotation() {
      // Tween attributes
      final zoomTween = Tween<double>(
          begin: widget.mapController.camera.rotation,
          end: 0);

      final controller = AnimationController(
          duration: const Duration(milliseconds: 500), vsync: this);

      Animation<double> animation =
      CurvedAnimation(parent: controller, curve: Curves.fastOutSlowIn);

      // This will make sure the mapController is moved on every tick
      controller.addListener(() {
        widget.mapController.rotate(zoomTween.evaluate(animation));
      });

      animation.addStatusListener((status) {
        if (status == AnimationStatus.completed) {
          controller.dispose();
        } else if (status == AnimationStatus.dismissed) {
          controller.dispose();
        }
      });

      controller.forward();
    }

}