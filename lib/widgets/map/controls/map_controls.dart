import 'package:flutter/material.dart';

class MapControls extends StatelessWidget {
  final List<Widget> controls;

  const MapControls({
    super.key,
    required this.controls,
  });

  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.of(context).size.width;
    final isMobile = width < 600;

    if(isMobile){
      return Positioned(
        top: 80,
        right: 16,
        child: Column(
          children: controls,
        ),
      );
    }else{
      return Positioned(
        bottom: 80,
        right: 16,
        child: Column(
          mainAxisAlignment: MainAxisAlignment.end,
          children: controls,
        ),
      );
    }
  }
}