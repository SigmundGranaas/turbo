import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';
import 'package:provider/provider.dart';

import 'location_provider.dart';

class LocationMarkers extends StatelessWidget {
  final Function(Map<String, dynamic>) onMarkerTap;

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
              point: LatLng(location['latitude'], location['longitude']),
              child: GestureDetector(
                onTap: () => onMarkerTap(location),
                child: Column(
                  children: [
                    const Icon(Icons.location_pin, color: Colors.blue),
                    Text(location['name'], style: const TextStyle(fontSize: 12)),
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