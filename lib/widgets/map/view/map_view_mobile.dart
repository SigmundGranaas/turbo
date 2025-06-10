import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/auth/drawer_widget.dart';
import 'package:map_app/widgets/map/controls/map_controls.dart';
import 'package:map_app/widgets/map/map_base.dart';
import 'package:map_app/widgets/search/search_bar_mobile.dart';

class MobileMapView extends StatelessWidget {
  final GlobalKey<ScaffoldState> scaffoldKey;
  final MapController mapController;
  final List<Widget> mapLayers;
  final List<Widget> mapControls;
  final Function(TapPosition, LatLng) onLongPress;
  final Function(double, double) onLocationSelected;
  final Marker? temporaryPin;

  const MobileMapView({
    super.key,
    required this.scaffoldKey,
    required this.mapController,
    required this.mapLayers,
    required this.mapControls,
    required this.onLongPress,
    required this.onLocationSelected,
    this.temporaryPin,
  });

  @override
  Widget build(BuildContext context) {
    final allMapLayers = [
      ...mapLayers,
      if (temporaryPin != null) MarkerLayer(markers: [temporaryPin!]),
    ];

    return Scaffold(
      key: scaffoldKey,
      drawer: const AppDrawer(),
      body: MapBase(
        mapController: mapController,
        mapLayers: allMapLayers,
        onLongPress: onLongPress,
        overlayWidgets: [
          MapControls(controls: mapControls),
          Positioned(
            top: 20,
            left: 0,
            right: 0,
            child: MobileSearchBar(
              onLocationSelected: onLocationSelected,
              onMenuPressed: () => scaffoldKey.currentState?.openDrawer(),
            ),
          ),
        ],
      ),
    );
  }
}