import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';
import 'package:turbo/core/data/database_provider.dart';

import '../models/collection.dart';
import '../models/collection_item_ref.dart';
import 'collection_data_store.dart';
import 'indexdb_collection_data_store.dart';
import 'sqlite_collection_data_store.dart';

final localCollectionDataStoreProvider =
    FutureProvider<CollectionDataStore>((ref) async {
  if (kIsWeb) {
    final store = ShimDBCollectionDataStore();
    await store.init();
    return store;
  } else {
    final db = await ref.watch(databaseProvider.future);
    return SQLiteCollectionDataStore(db);
  }
});

class CollectionRepositoryState {
  final List<Collection> collections;
  final Map<String, int> memberCounts;
  final Map<CollectionItemRef, List<String>> membershipIndex;

  const CollectionRepositoryState({
    required this.collections,
    required this.memberCounts,
    required this.membershipIndex,
  });

  const CollectionRepositoryState.empty()
      : collections = const [],
        memberCounts = const {},
        membershipIndex = const {};

  CollectionRepositoryState copyWith({
    List<Collection>? collections,
    Map<String, int>? memberCounts,
    Map<CollectionItemRef, List<String>>? membershipIndex,
  }) {
    return CollectionRepositoryState(
      collections: collections ?? this.collections,
      memberCounts: memberCounts ?? this.memberCounts,
      membershipIndex: membershipIndex ?? this.membershipIndex,
    );
  }
}

final collectionRepositoryProvider = NotifierProvider<CollectionRepository,
    AsyncValue<CollectionRepositoryState>>(() {
  return CollectionRepository();
});

class CollectionRepository
    extends Notifier<AsyncValue<CollectionRepositoryState>> {
  final _log = Logger('CollectionRepository');

  @override
  AsyncValue<CollectionRepositoryState> build() {
    _loadData();
    return const AsyncValue.data(CollectionRepositoryState.empty());
  }

  Future<CollectionDataStore> get _store =>
      ref.read(localCollectionDataStoreProvider.future);

  Future<void> _loadData() async {
    if (!ref.mounted) return;
    state = const AsyncValue.loading();
    try {
      final store = await _store;
      if (!ref.mounted) return;
      final collections = await store.getAll();
      if (!ref.mounted) return;
      final membershipIndex = await store.getMembershipIndex();
      if (!ref.mounted) return;
      final counts = <String, int>{};
      for (final entry in membershipIndex.entries) {
        for (final cUuid in entry.value) {
          counts[cUuid] = (counts[cUuid] ?? 0) + 1;
        }
      }
      state = AsyncValue.data(CollectionRepositoryState(
        collections: collections,
        memberCounts: counts,
        membershipIndex: membershipIndex,
      ));
    } catch (e, st) {
      _log.severe('Error loading collections', e, st);
      if (!ref.mounted) return;
      state = AsyncValue.error(e, st);
    }
  }

  Future<void> refresh() => _loadData();

  Future<Collection> create(Collection collection) async {
    final store = await _store;
    await store.insert(collection);
    await _loadData();
    return collection;
  }

  Future<void> updateCollection(Collection collection) async {
    final store = await _store;
    await store.update(collection);
    await _loadData();
  }

  Future<void> deleteCollection(String uuid) async {
    final store = await _store;
    await store.delete(uuid);
    await _loadData();
  }

  Future<void> addItem(String collectionUuid, CollectionItemRef ref) async {
    final store = await _store;
    await store.addItem(collectionUuid, ref);
    await _loadData();
  }

  Future<void> removeItem(String collectionUuid, CollectionItemRef ref) async {
    final store = await _store;
    await store.removeItem(collectionUuid, ref);
    await _loadData();
  }

  /// Set the exact set of collections an item belongs to.
  Future<void> setMembership(
    CollectionItemRef ref,
    Set<String> desiredCollectionUuids,
  ) async {
    final store = await _store;
    final current = (await store.getCollectionUuidsFor(ref)).toSet();
    final toAdd = desiredCollectionUuids.difference(current);
    final toRemove = current.difference(desiredCollectionUuids);
    for (final c in toAdd) {
      await store.addItem(c, ref);
    }
    for (final c in toRemove) {
      await store.removeItem(c, ref);
    }
    await _loadData();
  }

  /// Called by marker/path repositories when an underlying item is deleted,
  /// so dangling join rows do not accumulate.
  Future<void> handleItemDeleted(CollectionItemRef ref) async {
    final store = await _store;
    await store.removeItemFromAll(ref);
    await _loadData();
  }
}

extension CollectionRepositoryStateX on CollectionRepositoryState {
  List<String> collectionsFor(CollectionItemRef ref) =>
      membershipIndex[ref] ?? const [];

  int memberCountFor(String collectionUuid) =>
      memberCounts[collectionUuid] ?? 0;
}
