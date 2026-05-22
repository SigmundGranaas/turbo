import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'indexdb_saved_path_datastore.dart';
import 'saved_path_data_store.dart';
import 'sqlite_saved_path_datastore.dart';
import '../models/saved_path.dart';

final localSavedPathDataStoreProvider = FutureProvider<SavedPathDataStore>((ref) async {
  if (kIsWeb) {
    final store = ShimDBSavedPathDataStore();
    await store.init();
    return store;
  } else {
    final db = await ref.watch(databaseProvider.future);
    return SQLiteSavedPathDataStore(db);
  }
});

final savedPathRepositoryProvider = NotifierProvider.autoDispose<SavedPathRepository, AsyncValue<List<SavedPath>>>(() {
  return SavedPathRepository();
});

class SavedPathRepository extends Notifier<AsyncValue<List<SavedPath>>> {
  final _log = Logger('SavedPathRepository');

  @override
  AsyncValue<List<SavedPath>> build() {
    _loadData();
    return const AsyncValue.data([]);
  }

  Future<SavedPathDataStore> get _localStore => ref.read(localSavedPathDataStoreProvider.future);

  Future<void> _loadData() async {
    state = const AsyncValue.loading();
    try {
      final localStore = await _localStore;
      final paths = await localStore.getAll();
      state = AsyncValue.data(paths);
    } catch (e, st) {
      _log.severe("Error loading saved paths", e, st);
      state = AsyncValue.error(e, st);
    }
  }

  Future<void> addPath(SavedPath path) async {
    final localStore = await _localStore;
    try {
      await localStore.insert(path);
      await _loadData();
    } catch (e, st) {
      _log.warning("Error adding path", e, st);
      state = AsyncValue.error(e, st);
    }
  }

  Future<void> updatePath(SavedPath path) async {
    final localStore = await _localStore;
    try {
      await localStore.update(path);
      await _loadData();
    } catch (e, st) {
      _log.warning("Error updating path", e, st);
      state = AsyncValue.error(e, st);
    }
  }

  Future<void> deletePath(String uuid) async {
    final localStore = await _localStore;
    try {
      await localStore.delete(uuid);
      await _loadData();
    } catch (e, st) {
      _log.warning("Error deleting path", e, st);
      state = AsyncValue.error(e, st);
    }
  }
}
