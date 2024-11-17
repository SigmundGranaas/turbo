import 'package:riverpod_annotation/riverpod_annotation.dart';
import '../../datastore/factory.dart';
import '../../model/marker.dart';

part 'location_provider.g.dart';


@riverpod
class LocationNotifier extends _$LocationNotifier {
  @override
  FutureOr<List<Marker>> build() async {
    return _loadLocations();
  }

  Future<List<Marker>> _loadLocations() async {
    return await MarkerDataStoreFactory.getDataStore().getAll();
  }

  Future<void> addLocation(Marker location) async {
    state = const AsyncLoading();
    state = await AsyncValue.guard(() async {
      await MarkerDataStoreFactory.getDataStore().insert(location);
      return _loadLocations();
    });
  }

  Future<void> updateLocation(Marker location) async {
    state = const AsyncLoading();
    state = await AsyncValue.guard(() async {
      await MarkerDataStoreFactory.getDataStore().update(location);
      return _loadLocations();
    });
  }

  Future<void> deleteLocation(String id) async {
    state = const AsyncLoading();
    state = await AsyncValue.guard(() async {
      await MarkerDataStoreFactory.getDataStore().delete(id);
      return _loadLocations();
    });
  }
}