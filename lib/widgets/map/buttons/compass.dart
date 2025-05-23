import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'map_control_button_base.dart';

class CustomMapCompass extends StatefulWidget {
  final MapController mapController;
  final String _svg = '''
  '<svg  viewBox="0 0 35 34" xmlns="http://www.w3.org/2000/svg">
      <g clip-path="url(#clip0_52_1221)">
  <path d="M15.6871 6.10915L11.914 14.6906C11.6013 15.4001 11.4389 16.1665 11.4368 16.9418C11.4347 17.7171 11.5931 18.4845 11.902 19.1956L15.6836 27.8831C15.8245 28.2079 16.0576 28.4842 16.354 28.6778C16.6504 28.8714 16.9972 28.9737 17.3512 28.9722C17.7053 28.9706 18.0511 28.8651 18.3458 28.6689C18.6405 28.4727 18.8711 28.1943 19.0091 27.8682L22.7065 19.1447C23.0018 18.4475 23.1532 17.6979 23.1516 16.9407C23.1501 16.1836 22.9955 15.4346 22.6973 14.7387L19.007 6.12542C18.8683 5.80097 18.6378 5.52416 18.3438 5.32905C18.0498 5.13395 17.7052 5.02908 17.3524 5.02735C16.9995 5.02562 16.6539 5.12711 16.358 5.31933C16.0621 5.51154 15.8289 5.78608 15.6871 6.10915ZM18.227 17.8839C18.0522 18.0587 17.8295 18.1778 17.587 18.226C17.3445 18.2742 17.0932 18.2495 16.8648 18.1549C16.6364 18.0603 16.4412 17.9 16.3038 17.6945C16.1664 17.4889 16.0931 17.2472 16.0931 17C16.0931 16.7528 16.1664 16.5111 16.3038 16.3056C16.4412 16.1 16.6364 15.9398 16.8648 15.8452C17.0932 15.7506 17.3445 15.7258 17.587 15.774C17.8295 15.8223 18.0522 15.9413 18.227 16.1161C18.4614 16.3506 18.5931 16.6685 18.5931 17C18.5931 17.3315 18.4614 17.6495 18.227 17.8839Z" fill="#797676"/>
  </g>
  <defs>
  <clipPath id="clip0_52_1221">
  <rect width="24" height="24" fill="white" transform="translate(0.372559 17) rotate(-45)"/>
  </clipPath>
  </defs>
  </svg>
  ''';

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
    final IconThemeData iconTheme = IconTheme.of(context);
    final colorScheme = Theme.of(context).colorScheme;

    return MapControlButtonBase(
      onPressed: _resetRotation,
      child: Transform.rotate(
        angle: -_rotation * (pi / 180),
        child: SvgPicture.string(
          widget._svg,
          width: iconTheme.size,
          height: iconTheme.size,
          // Use the current theme's icon color
          colorFilter: ColorFilter.mode(
            colorScheme.primary,
            BlendMode.srcIn,
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
