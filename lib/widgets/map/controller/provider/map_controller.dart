import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

final mapControllerProvProvider =
NotifierProvider.autoDispose<MapControllerProv, MapControllerState>(
  MapControllerProv.new,
);

class MapControllerProv extends Notifier<MapControllerState> {
  @override
  MapControllerState build() {
    return MapControllerState(MapController());
  }

  MapController controller() {
    return state.controller;
  }
}

class MapControllerState {
  final MapController controller;

  MapControllerState(this.controller);
}