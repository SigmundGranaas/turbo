import 'package:latlong2/latlong.dart';
import '../models/saved_path.dart';

abstract class SavedPathDataStore {
  Future<void> init();
  Future<void> insert(SavedPath path);
  Future<SavedPath?> getByUuid(String uuid);
  Future<List<SavedPath>> getAll();
  Future<void> update(SavedPath path);
  Future<void> delete(String uuid);
  Future<List<SavedPath>> findInBounds(LatLng southwest, LatLng northeast);
}
