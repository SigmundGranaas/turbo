import 'package:flutter/foundation.dart';
import 'database_helper.dart';

class LocationProvider extends ChangeNotifier {
  List<Map<String, dynamic>> _locations = [];

  List<Map<String, dynamic>> get locations => _locations;

  Future<void> loadLocations() async {
    _locations = await DatabaseHelper.queryAllLocations();
    notifyListeners();
  }

  Future<void> addLocation(Map<String, dynamic> location) async {
    await DatabaseHelper.insertLocation(location);
    await loadLocations();
  }

  Future<void> updateLocation(Map<String, dynamic> location) async {
    await DatabaseHelper.updateLocation(location);
    await loadLocations();
  }

  Future<void> deleteLocation(int id) async {
    await DatabaseHelper.deleteLocation(id);
    await loadLocations();
  }
}