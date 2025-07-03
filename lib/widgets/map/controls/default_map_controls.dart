import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';

import '../../../features/map_view/widgets/buttons/map_layer_button.dart';
import '../buttons/compass.dart';
import '../buttons/location_button.dart';
import '../buttons/plus_minus_buttons.dart';
import '../controller/map_utility.dart';

List<Widget> defaultMapControls(MapController controller, TickerProvider ticker) {
  final controls = [
    const MapLayerButton(),
    LocationButton(mapController: controller),
    CustomMapCompass(mapController: controller),
    PlusMinusButtons(
      onZoomIn: () => zoomIn(controller, ticker),
      onZoomOut: () => zoomOut(controller, ticker),
    ),
  ];
  return controls;
}

List<Widget> defaultMobileMapControls(MapController controller, TickerProvider ticker) {
  final controls = [
    const MapLayerButton(),
    LocationButton(mapController: controller),
    CustomMapCompass(mapController: controller),
  ];
  return controls;
}