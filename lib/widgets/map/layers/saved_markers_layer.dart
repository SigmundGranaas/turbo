import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:provider/provider.dart';
import 'package:map_app/data/model/marker.dart' as marker_model;

import '../../../data/icon_service.dart';
import '../../../data/model/named_icon.dart';
import '../../../location_provider.dart';
import '../../marker/edit_location_sheet.dart';

class LocationMarkers extends StatelessWidget {
  final Function(marker_model.Marker) onMarkerTap;

  const LocationMarkers({super.key, required this.onMarkerTap});

  @override
  Widget build(BuildContext context) {
    final iconService = IconService();  // Create an instance of IconService

    return Consumer<LocationProvider>(
      builder: (context, locationProvider, child) {
        return MarkerLayer(
          markers: locationProvider.locations.map((location) {
            final namedIcon = iconService.getIcon(location.icon);  // Get the NamedIcon
            return Marker(
              width: 80.0,
              height: 80.0,
              point: location.position,
              child: MapIcon(
                namedIcon: namedIcon,
                title: location.title,
                onTap: () {
                  _showEditSheet(context, location);
                },
              )
            );
          }).toList(),
        );
      },
    );
  }
  void _showEditSheet(BuildContext context, marker_model.Marker marker) async {
    await showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      builder: (BuildContext context) {
        return EditLocationSheet(location: marker);
      },
    );
  }
}


class MapIcon extends StatefulWidget {
  final NamedIcon namedIcon;
  final String title;
  final VoidCallback onTap;

  const MapIcon({
    super.key,
    required this.namedIcon,
    required this.title,
    required this.onTap,
  });

  @override
  State<MapIcon> createState() => _MapIconState();
}

class _MapIconState extends State<MapIcon> {
  bool _isHovering = false;
  bool _isPressed = false;

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _isHovering = true),
      onExit: (_) => setState(() => _isHovering = false),
      child: GestureDetector(
        onTapDown: (_) => setState(() => _isPressed = true),
        onTapUp: (_) => setState(() => _isPressed = false),
        onTapCancel: () => setState(() => _isPressed = false),
        onTap: widget.onTap,
        child: Column(
          children: [
            AnimatedContainer(
              duration: const Duration(milliseconds: 100),
              width: 40, // Adjust size as needed
              height: 40, // Adjust size as needed
              decoration: BoxDecoration(
                color: _isHovering || _isPressed ? Colors.blue.shade100 : Colors.white,
                shape: BoxShape.circle,
                boxShadow: [
                  BoxShadow(
                    color: Colors.black.withOpacity(0.2),
                    blurRadius: 4,
                    offset: const Offset(0, 2),
                  ),
                ],
                border: Border.all(
                  color: _isHovering || _isPressed ? Colors.blue : Colors.transparent,
                  width: 2,
                ),
              ),
              child: Center(
                child: Icon(
                  widget.namedIcon.icon,
                  color: _isHovering || _isPressed ? Colors.blue.shade700 : Colors.blue,
                  size: 24, // Adjust size as needed
                ),
              ),
            ),
            const SizedBox(height: 4), // Space between icon and text
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 2),
              decoration: BoxDecoration(
                color: Colors.white.withOpacity(0.8),
                borderRadius: BorderRadius.circular(4),
              ),
              child: Text(
                widget.title,
                style: TextStyle(
                  fontSize: 12,
                  color: Colors.blue.shade700 ,
                  fontWeight: FontWeight.bold,
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        ),
      ),
    );
  }
}