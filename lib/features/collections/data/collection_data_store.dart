import '../models/collection.dart';
import '../models/collection_item_ref.dart';

abstract class CollectionDataStore {
  Future<void> init();

  // Collections
  Future<List<Collection>> getAll();
  Future<Collection?> getByUuid(String uuid);
  Future<void> insert(Collection collection);
  Future<void> update(Collection collection);
  Future<void> delete(String uuid);

  // Membership
  Future<List<CollectionItemRef>> getItems(String collectionUuid);
  Future<List<String>> getCollectionUuidsFor(CollectionItemRef ref);
  Future<Map<CollectionItemRef, List<String>>> getMembershipIndex();
  Future<void> addItem(String collectionUuid, CollectionItemRef ref);
  Future<void> removeItem(String collectionUuid, CollectionItemRef ref);
  Future<void> removeItemFromAll(CollectionItemRef ref);
  Future<int> countItems(String collectionUuid);
}
