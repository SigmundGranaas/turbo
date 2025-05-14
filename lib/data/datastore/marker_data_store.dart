import 'package:latlong2/latlong.dart';
import '../model/marker.dart';

abstract class MarkerDataStore {
  Future<void> init();
  Future<void> insert(Marker marker);
  Future<Marker?> getByUuid(String uuid);
  Future<List<Marker>> getAll();
  Future<List<Marker>> getUnsynced();
  Future<void> update(Marker marker);
  Future<void> delete(String uuid);
  Future<void>deleteAll(List<String> uuids);
  Future<void> clearAll();
  Future<List<Marker>> findInBounds(LatLng southwest, LatLng northeast);
}