import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/features/tile_providers/models/custom_tile_provider.dart';

final customProviderStoreProvider =
    AsyncNotifierProvider<CustomProviderStore, List<CustomTileProvider>>(
        CustomProviderStore.new);

/// Persistence for user-defined tile providers. JSON-encoded list under a
/// single SharedPreferences key, modelled after [LayerPreferenceService].
class CustomProviderStore extends AsyncNotifier<List<CustomTileProvider>> {
  static const _key = 'custom_tile_providers';

  @override
  Future<List<CustomTileProvider>> build() async {
    final prefs = await SharedPreferences.getInstance();
    return CustomTileProvider.decodeList(prefs.getString(_key));
  }

  Future<void> add(CustomTileProvider provider) async {
    final current = state.value ?? const <CustomTileProvider>[];
    final next = [...current, provider];
    await _persist(next);
    state = AsyncData(next);
  }

  Future<void> remove(String id) async {
    final current = state.value ?? const <CustomTileProvider>[];
    final next = current.where((p) => p.id != id).toList();
    await _persist(next);
    state = AsyncData(next);
  }

  Future<void> _persist(List<CustomTileProvider> providers) async {
    final prefs = await SharedPreferences.getInstance();
    if (providers.isEmpty) {
      await prefs.remove(_key);
    } else {
      await prefs.setString(_key, CustomTileProvider.encodeList(providers));
    }
  }
}
