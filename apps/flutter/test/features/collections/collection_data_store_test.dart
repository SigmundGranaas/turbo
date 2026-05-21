import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/features/collections/data/sqlite_collection_data_store.dart';
import 'package:turbo/features/collections/models/collection.dart';
import 'package:turbo/features/collections/models/collection_item_ref.dart';

import '../../helpers/in_memory_db.dart';

void main() {
  late Database db;
  late SQLiteCollectionDataStore store;

  setUp(() async {
    db = await createCollectionsDb();
    store = SQLiteCollectionDataStore(db);
  });

  tearDown(() async {
    await db.close();
  });

  test('insert / getAll / getByUuid', () async {
    final c = Collection(name: 'Trip A', description: 'd', colorHex: 'FF0000');
    await store.insert(c);
    final all = await store.getAll();
    expect(all, hasLength(1));
    expect(all.first.uuid, c.uuid);

    final found = await store.getByUuid(c.uuid);
    expect(found?.name, 'Trip A');
    expect(found?.colorHex, 'FF0000');
  });

  test('update changes existing collection', () async {
    final c = Collection(name: 'A');
    await store.insert(c);
    await store.update(c.copyWith(name: 'B'));
    final found = await store.getByUuid(c.uuid);
    expect(found?.name, 'B');
  });

  test('delete removes collection and its join rows', () async {
    final c = Collection(name: 'A');
    await store.insert(c);
    final m = CollectionItemRef(type: 'marker', uuid: 'm1');
    await store.addItem(c.uuid, m);
    await store.delete(c.uuid);
    expect(await store.getByUuid(c.uuid), isNull);
    expect(await store.getCollectionUuidsFor(m), isEmpty);
  });

  test('addItem is idempotent (PK conflict ignored)', () async {
    final c = Collection(name: 'A');
    await store.insert(c);
    final m = CollectionItemRef(type: 'marker', uuid: 'm1');
    await store.addItem(c.uuid, m);
    await store.addItem(c.uuid, m);
    expect(await store.countItems(c.uuid), 1);
  });

  test('many-to-many membership for the same item', () async {
    final a = Collection(name: 'A');
    final b = Collection(name: 'B');
    await store.insert(a);
    await store.insert(b);
    final m = CollectionItemRef(type: 'marker', uuid: 'm1');
    await store.addItem(a.uuid, m);
    await store.addItem(b.uuid, m);
    final cs = await store.getCollectionUuidsFor(m);
    expect(cs.toSet(), {a.uuid, b.uuid});
  });

  test('removeItem only removes from the target collection', () async {
    final a = Collection(name: 'A');
    final b = Collection(name: 'B');
    await store.insert(a);
    await store.insert(b);
    final m = CollectionItemRef(type: 'marker', uuid: 'm1');
    await store.addItem(a.uuid, m);
    await store.addItem(b.uuid, m);
    await store.removeItem(a.uuid, m);
    expect((await store.getCollectionUuidsFor(m)).toSet(), {b.uuid});
  });

  test('removeItemFromAll clears every join row for an item', () async {
    final a = Collection(name: 'A');
    final b = Collection(name: 'B');
    await store.insert(a);
    await store.insert(b);
    final m = CollectionItemRef(type: 'marker', uuid: 'm1');
    await store.addItem(a.uuid, m);
    await store.addItem(b.uuid, m);
    await store.removeItemFromAll(m);
    expect(await store.getCollectionUuidsFor(m), isEmpty);
  });

  test('getItems returns markers and paths mixed', () async {
    final c = Collection(name: 'Mixed');
    await store.insert(c);
    await store.addItem(c.uuid,
        CollectionItemRef(type: CollectionItemRef.typeMarker, uuid: 'm1'));
    await store.addItem(c.uuid,
        CollectionItemRef(type: CollectionItemRef.typePath, uuid: 'p1'));
    final items = await store.getItems(c.uuid);
    expect(items, hasLength(2));
    expect(items.map((r) => r.type).toSet(),
        {CollectionItemRef.typeMarker, CollectionItemRef.typePath});
  });

  test('getMembershipIndex builds reverse lookup', () async {
    final a = Collection(name: 'A');
    final b = Collection(name: 'B');
    await store.insert(a);
    await store.insert(b);
    final m1 = CollectionItemRef(type: 'marker', uuid: 'm1');
    final m2 = CollectionItemRef(type: 'marker', uuid: 'm2');
    await store.addItem(a.uuid, m1);
    await store.addItem(b.uuid, m1);
    await store.addItem(a.uuid, m2);
    final idx = await store.getMembershipIndex();
    expect(idx[m1]!.toSet(), {a.uuid, b.uuid});
    expect(idx[m2]!.toSet(), {a.uuid});
  });
}
