import '../models/collection_item_ref.dart';
import 'collection_repository.dart';

/// Returns true when an item should be visible on the map given the current
/// collection memberships and visibility map.
///
/// Semantics: an item is hidden only when it belongs to one or more
/// collections AND every one of those collections is toggled off. An item in
/// zero collections is always visible (subject to the global type toggles).
bool isItemVisibleForCollections({
  required CollectionItemRef ref,
  required CollectionRepositoryState collectionState,
  required Map<String, bool> visibility,
}) {
  final memberships = collectionState.collectionsFor(ref);
  if (memberships.isEmpty) return true;
  for (final c in memberships) {
    if (visibility[c] ?? true) return true;
  }
  return false;
}
