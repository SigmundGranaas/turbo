import 'package:flutter_map/flutter_map.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'map_controller.g.dart';


@riverpod
class MapControllerProv extends _$MapControllerProv {
  @override
  MapControllerState build() {
    return MapControllerState(MapController());
  }

  MapController controller(){
    return state.controller;
  }
}

class MapControllerState {
  final MapController controller;

  MapControllerState(this.controller);
}
