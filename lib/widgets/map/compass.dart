import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'dart:math' as math;

class CustomCompass extends StatefulWidget {
  final MapController mapController;

  const CustomCompass({super.key, required this.mapController});

  @override
  State<CustomCompass> createState() => _CustomCompassState();
}

class _CustomCompassState extends State<CustomCompass>
    with TickerProviderStateMixin {
  late AnimationController _animationController;
  late Animation<double> _rotationAnimation;
  double _rotation = 0.0;

  @override
  void initState() {
    super.initState();
    _animationController = AnimationController(
      duration: const Duration(milliseconds: 300),
      vsync: this,
    );
    _rotationAnimation =
        Tween<double>(begin: 0, end: 1).animate(_animationController);
    widget.mapController.mapEventStream.listen(_onMapEvent);
  }

  void _onMapEvent(MapEvent mapEvent) {
    if (mapEvent is MapEventRotate) {
      setState(() {
        _rotation = mapEvent.camera.rotation;
      });
    }
  }

  void _resetRotation() {
    final currentRotation = widget.mapController.camera.rotation;
    _rotationAnimation = Tween<double>(
      begin: currentRotation,
      end: 0.0,
    ).animate(CurvedAnimation(
      parent: _animationController,
      curve: Curves.easeInOut,
    ));

    _animationController.forward(from: 0).then((_) {
      widget.mapController.rotate(0);
    });
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
        animation: _rotationAnimation,
        builder: (context, child) {
          return Card(
            elevation: 4,
            child: Padding(
              padding: const EdgeInsets.all(8.0),
              child: Transform.rotate(
                angle: -(_rotationAnimation.value) * (math.pi / 180.0),
                child: IconButton(
                    icon: const Icon(Icons.compass_calibration_sharp),
                    onPressed: _resetRotation),
              ),
            ),
          );
        });
  }

  @override
  void dispose() {
    _animationController.dispose();
    super.dispose();
  }
}
