import 'package:latlong2/latlong.dart';

class NavigationState {
  final LatLng? target;
  final bool isActive;

  const NavigationState({this.target, this.isActive = false});

  static const inactive = NavigationState();
}
