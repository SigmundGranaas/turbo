import 'package:flutter/foundation.dart';
import 'data/datastore/factory.dart';
import 'data/model/marker.dart';

class LocationProvider extends ChangeNotifier {
  List<Marker> _locations = [];

  List<Marker> get locations => _locations;

  Future<void> loadLocations() async {
    _locations = await MarkerDataStoreFactory.getDataStore().getAll();
    notifyListeners();
  }

  Future<void> addLocation(Marker location) async {
    await MarkerDataStoreFactory.getDataStore().insert(location);
    await loadLocations();
  }

  Future<void> updateLocation(Marker location) async {
    await MarkerDataStoreFactory.getDataStore().update(location);
    await loadLocations();
  }

  Future<void> deleteLocation(String id) async {
    await MarkerDataStoreFactory.getDataStore().delete(id);
    await loadLocations();
  }
}