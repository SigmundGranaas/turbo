import 'package:flutter/material.dart';

class MapControlButtonBase extends StatelessWidget {
  final Widget child;
  final VoidCallback onPressed;

  const MapControlButtonBase({
    super.key,
    required this.child,
    required this.onPressed,
  });

  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.of(context).size.width;
    final isMobile = width < 600;

    final buttonSize = isMobile ? 48.0 : 64.0;
    final paddingSize = isMobile ? 4.0 : 8.0;

    return SizedBox(
      width: buttonSize,
      height: buttonSize,
      child: Card(
        elevation: 4,
        shape: const CircleBorder(),
        child: ClipOval(
          child: Material(
            color: Theme.of(context).cardColor,
            child: InkWell(
              onTap: onPressed,
              child: Padding(
                padding: EdgeInsets.all(paddingSize),
                child: child,
              ),
            ),
          ),
        ),
      ),
    );
  }
}