import 'package:latlong2/latlong.dart';

import 'measure_point_type.dart';

class MeasurePoint {
  final LatLng point;
  final MeasurePointType type;

  MeasurePoint({
    required this.point,
    required this.type,
  });
}