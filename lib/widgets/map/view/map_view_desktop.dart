import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/auth/drawer_widget.dart';
import 'package:map_app/widgets/map/controls/map_controls.dart';
import 'package:map_app/widgets/map/map_base.dart';
import 'package:map_app/widgets/search/search_bar_desktop.dart';

class DesktopMapView extends StatelessWidget {
  final GlobalKey<ScaffoldState> scaffoldKey;
  final MapController mapController;
  final TickerProvider tickerProvider;
  final List<Widget> mapLayers;
  final List<Widget> mapControls;
  final Function(TapPosition, LatLng) onLongPress;
  final Marker? temporaryPin;

  const DesktopMapView({
    super.key,
    required this.scaffoldKey,
    required this.mapController,
    required this.tickerProvider,
    required this.mapLayers,
    required this.mapControls,
    required this.onLongPress,
    this.temporaryPin,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
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
            top: 16,
            left: 16 + 64 + 16, // Padded to the right of the menu button
            child: DesktopSearchBar(
              mapController: mapController,
              tickerProvider: tickerProvider,
            ),
          ),
          Positioned(
            left: 16,
            top: 16,
            child: SizedBox(
              width: 64,
              height: 64,
              child: Card(
                elevation: 4,
                shape: const CircleBorder(),
                child: ClipOval(
                  child: Material(
                    color: colorScheme.surface,
                    child: InkWell(
                      onTap: () => scaffoldKey.currentState?.openDrawer(),
                      child: Padding(
                        padding: const EdgeInsets.all(8),
                        child: Icon(Icons.menu, color: colorScheme.primary),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}