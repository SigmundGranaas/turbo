import 'package:uuid/uuid.dart';

class Collection {
  final String uuid;
  final String name;
  final String? description;
  final String? colorHex;
  final String? iconKey;
  final DateTime createdAt;
  final int sortOrder;

  Collection({
    String? uuid,
    required this.name,
    this.description,
    this.colorHex,
    this.iconKey,
    DateTime? createdAt,
    this.sortOrder = 0,
  })  : uuid = uuid ?? const Uuid().v4(),
        createdAt = createdAt ?? DateTime.now();

  Collection copyWith({
    String? uuid,
    String? name,
    String? description,
    bool clearDescription = false,
    String? colorHex,
    bool clearColorHex = false,
    String? iconKey,
    bool clearIconKey = false,
    DateTime? createdAt,
    int? sortOrder,
  }) {
    return Collection(
      uuid: uuid ?? this.uuid,
      name: name ?? this.name,
      description:
          clearDescription ? null : (description ?? this.description),
      colorHex: clearColorHex ? null : (colorHex ?? this.colorHex),
      iconKey: clearIconKey ? null : (iconKey ?? this.iconKey),
      createdAt: createdAt ?? this.createdAt,
      sortOrder: sortOrder ?? this.sortOrder,
    );
  }

  factory Collection.fromLocalMap(Map<String, dynamic> map) {
    return Collection(
      uuid: map['uuid'] as String,
      name: map['name'] as String,
      description: map['description'] as String?,
      colorHex: map['color_hex'] as String?,
      iconKey: map['icon_key'] as String?,
      createdAt: DateTime.parse(map['created_at'] as String),
      sortOrder: (map['sort_order'] as num?)?.toInt() ?? 0,
    );
  }

  Map<String, dynamic> toLocalMap() {
    return {
      'uuid': uuid,
      'name': name,
      'description': description,
      'color_hex': colorHex,
      'icon_key': iconKey,
      'created_at': createdAt.toIso8601String(),
      'sort_order': sortOrder,
    };
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Collection &&
          runtimeType == other.runtimeType &&
          uuid == other.uuid &&
          name == other.name &&
          description == other.description &&
          colorHex == other.colorHex &&
          iconKey == other.iconKey &&
          createdAt == other.createdAt &&
          sortOrder == other.sortOrder;

  @override
  int get hashCode => Object.hash(
        uuid,
        name,
        description,
        colorHex,
        iconKey,
        createdAt,
        sortOrder,
      );
}
