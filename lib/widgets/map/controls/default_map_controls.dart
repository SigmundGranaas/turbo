import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';

import '../buttons/compass.dart';
import '../buttons/location_button.dart';
import '../buttons/map_layer_button.dart';
import '../buttons/plus_minus_buttons.dart';
import '../controller/map_utility.dart';

List<Widget> defaultMapControls(MapController controller, TickerProvider ticker) {
  final controls = [
    const MapLayerButton(),
    const LocationButton(),
    CustomMapCompass(mapController: controller),
    PlusMinusButtons(
      onZoomIn: () => zoomIn(controller, ticker),
      onZoomOut: () => zoomOut(controller, ticker),
    ),
  ];
  return controls;
}
