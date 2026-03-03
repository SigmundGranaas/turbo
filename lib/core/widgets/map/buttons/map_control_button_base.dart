import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class MapControlButtonBase extends StatelessWidget {
  final Widget child;
  final VoidCallback onPressed;
  final VoidCallback? onLongPress;
  final bool isActive;

  const MapControlButtonBase({
    super.key,
    required this.child,
    required this.onPressed,
    this.onLongPress,
    this.isActive = false,
  });

  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.of(context).size.width;
    final isMobile = width < 600;
    final colorScheme = Theme.of(context).colorScheme;

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
            color: isActive
                ? colorScheme.tertiaryContainer
                : colorScheme.surfaceContainer,
            child: InkWell(
              customBorder: const CircleBorder(),
              onTap: onPressed,
              onLongPress: onLongPress != null
                  ? () {
                      HapticFeedback.mediumImpact();
                      onLongPress!();
                    }
                  : null,
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