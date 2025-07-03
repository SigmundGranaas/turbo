import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/map_view/widgets/map_base.dart';
import 'package:turbo/widgets/auth/drawer_widget.dart';
import 'package:turbo/widgets/search/search_bar_mobile.dart';

import '../../../../widgets/map/controls/map_controls.dart';

class MobileMapView extends StatelessWidget {
  final GlobalKey<ScaffoldState> scaffoldKey;
  final MapController mapController;
  final TickerProvider tickerProvider;
  final List<Widget> mapLayers;
  final List<Widget> mapControls;
  final List<Widget> overlayWidgets;
  final Function(TapPosition, LatLng) onLongPress;
  final Marker? temporaryPin;
  final LatLng initialCenter;
  final double initialZoom;
  final MapEventCallback? onMapEvent;

  const MobileMapView({
    super.key,
    required this.scaffoldKey,
    required this.mapController,
    required this.tickerProvider,
    required this.mapLayers,
    required this.mapControls,
    required this.overlayWidgets,
    required this.onLongPress,
    this.temporaryPin,
    required this.initialCenter,
    required this.initialZoom,
    this.onMapEvent,
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
        initialCenter: initialCenter,
        initialZoom: initialZoom,
        onMapEvent: onMapEvent,
        overlayWidgets: [
          ...overlayWidgets,
          MapControls(controls: mapControls),
          SafeArea(
            child: Align(
              alignment: Alignment.topCenter,
              child: Padding(
                padding: const EdgeInsets.only(top: 8.0),
                child: MobileSearchBar(
                  mapController: mapController,
                  tickerProvider: tickerProvider,
                  onMenuPressed: () => scaffoldKey.currentState?.openDrawer(),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}