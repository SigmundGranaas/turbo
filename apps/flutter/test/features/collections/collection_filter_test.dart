import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/collections/data/collection_filter.dart';
import 'package:turbo/features/collections/data/collection_repository.dart';
import 'package:turbo/features/collections/models/collection_item_ref.dart';

CollectionRepositoryState _stateWith(Map<CollectionItemRef, List<String>> idx) {
  return CollectionRepositoryState(
    collections: const [],
    memberCounts: const {},
    membershipIndex: idx,
  );
}

void main() {
  const item = CollectionItemRef(type: 'marker', uuid: 'm1');

  test('item with no memberships is always visible', () {
    final visible = isItemVisibleForCollections(
      ref: item,
      collectionState: _stateWith({}),
      visibility: const {'A': false, 'B': false},
    );
    expect(visible, isTrue);
  });

  test('item is visible when at least one of its collections is visible', () {
    final state = _stateWith({
      item: ['A', 'B'],
    });
    expect(
      isItemVisibleForCollections(
        ref: item,
        collectionState: state,
        visibility: const {'A': false, 'B': true},
      ),
      isTrue,
    );
  });

  test('item is hidden when all of its collections are toggled off', () {
    final state = _stateWith({
      item: ['A', 'B'],
    });
    expect(
      isItemVisibleForCollections(
        ref: item,
        collectionState: state,
        visibility: const {'A': false, 'B': false},
      ),
      isFalse,
    );
  });

  test('missing visibility entry defaults to visible', () {
    final state = _stateWith({
      item: ['A'],
    });
    expect(
      isItemVisibleForCollections(
        ref: item,
        collectionState: state,
        visibility: const {},
      ),
      isTrue,
    );
  });
}
