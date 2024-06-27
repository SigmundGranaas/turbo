import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:provider/provider.dart';
import 'package:map_app/data/model/marker.dart' as marker_model;

import 'location_provider.dart';

class LocationMarkers extends StatelessWidget {
  final Function(marker_model.Marker) onMarkerTap;

  const LocationMarkers({super.key, required this.onMarkerTap});

  @override
  Widget build(BuildContext context) {
    return Consumer<LocationProvider>(
      builder: (context, locationProvider, child) {
        return MarkerLayer(
          markers: locationProvider.locations.map((location) {
            return Marker(
              width: 80.0,
              height: 80.0,
              point:  location.position,
              child: GestureDetector(
                onTap: () => onMarkerTap(location),
                child: Column(
                  children: [
                    const Icon(Icons.location_pin, color: Colors.blue),
                    Text(location.title, style: const TextStyle(fontSize: 12)),
                  ],
                ),
              ),
            );
          }).toList(),
        );
      },
    );
  }
}