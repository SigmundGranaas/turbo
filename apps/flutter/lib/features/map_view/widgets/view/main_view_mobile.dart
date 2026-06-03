import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/map_view/widgets/map_base.dart';
import 'package:turbo/features/auth/api.dart';
import 'package:turbo/features/search/api.dart';

import 'package:turbo/core/widgets/map/controls/map_controls.dart';

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

  /// Set while a map tool consumes taps (e.g. dropping route waypoints).
  final Function(TapPosition, LatLng)? onTap;

  /// Overrides map interaction while a tool is active (e.g. freezing pan).
  final InteractionOptions? interactionOptions;

  /// Hide the search bar while a full-screen tool overlay is mounted.
  final bool hideSearchBar;

  /// Forwarded to the search bar: selects a picked result so the shared detail
  /// sheet appears.
  final void Function(LocationSearchResult result)? onResultSelected;

  /// Raw pointer handlers for tools that draw freehand / drag-select.
  final void Function(PointerDownEvent, LatLng)? onPointerDown;
  final void Function(PointerMoveEvent, LatLng)? onPointerMove;
  final void Function(PointerUpEvent, LatLng)? onPointerUp;

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
    this.onTap,
    this.interactionOptions,
    this.hideSearchBar = false,
    this.onResultSelected,
    this.onPointerDown,
    this.onPointerMove,
    this.onPointerUp,
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
        onTap: onTap,
        onPointerDown: onPointerDown,
        onPointerMove: onPointerMove,
        onPointerUp: onPointerUp,
        interactionOptions: interactionOptions,
        initialCenter: initialCenter,
        initialZoom: initialZoom,
        onMapEvent: onMapEvent,
        overlayWidgets: [
          ...overlayWidgets,
          MapControls(controls: mapControls),
          if (!hideSearchBar)
            Positioned(
              top: 8,
              left: 0,
              right: 0,
              child: MobileSearchBar(
                mapController: mapController,
                tickerProvider: tickerProvider,
                onMenuPressed: () => scaffoldKey.currentState?.openDrawer(),
                onResultSelected: onResultSelected,
              ),
            ),
        ],
      ),
    );
  }
}