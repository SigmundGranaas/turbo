import '../model/marker.dart';

abstract class MarkerDataStore {
  Future<void> init();
  Future<void> insert(Marker marker);
  Future<Marker?> getByUuid(String uuid);
  Future<List<Marker>> findByName(String name);
  Future<List<Marker>> getAll();
  Future<void> update(Marker marker);
  Future<void> delete(String uuid);
}