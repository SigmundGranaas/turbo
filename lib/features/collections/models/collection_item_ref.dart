/// A typed reference to an item that can live in a [Collection].
///
/// Keeping the item type a free-form string keeps the join table generic:
/// adding a new item type (e.g. regions, photos) does not require a schema
/// change.
class CollectionItemRef {
  final String type;
  final String uuid;

  const CollectionItemRef({required this.type, required this.uuid});

  static const String typeMarker = 'marker';
  static const String typePath = 'path';

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is CollectionItemRef &&
          runtimeType == other.runtimeType &&
          type == other.type &&
          uuid == other.uuid;

  @override
  int get hashCode => Object.hash(type, uuid);

  @override
  String toString() => 'CollectionItemRef($type, $uuid)';
}
