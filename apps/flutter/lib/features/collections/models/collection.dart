import 'package:uuid/uuid.dart';

import 'saved_filter.dart';

class Collection {
  final String uuid;
  final String name;
  final String? description;
  final String? colorHex;
  final String? iconKey;
  final DateTime createdAt;
  final int sortOrder;

  /// When non-null, the collection is a "smart" collection — membership is
  /// computed from this filter on demand instead of from the explicit
  /// `collection_items` join table.
  final SavedFilter? savedFilter;

  /// Sync state: true once the row has been successfully written to the
  /// server's read model. Locally-created collections default to false;
  /// the sync orchestrator flips this to true after upload.
  final bool synced;

  /// Server-stamped monotonic version. Sent back as `If-Match` on
  /// update/delete/add-item/remove-item; null while the collection has
  /// not yet synced.
  final int? version;

  /// Server-stamped wall-clock of the last successful projection write.
  /// Drives the next `?since=` cursor for delta-sync.
  final DateTime? updatedAt;

  /// Server-side tombstone. The client uses this to recognise deletions
  /// learnt via delta-sync; always null in the local store for live rows.
  final DateTime? deletedAt;

  Collection({
    String? uuid,
    required this.name,
    this.description,
    this.colorHex,
    this.iconKey,
    DateTime? createdAt,
    this.sortOrder = 0,
    this.savedFilter,
    this.synced = false,
    this.version,
    this.updatedAt,
    this.deletedAt,
  })  : uuid = uuid ?? const Uuid().v4(),
        createdAt = createdAt ?? DateTime.now();

  bool get isSmart => savedFilter != null;

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
    SavedFilter? savedFilter,
    bool clearSavedFilter = false,
    bool? synced,
    int? version,
    bool clearVersion = false,
    DateTime? updatedAt,
    bool clearUpdatedAt = false,
    DateTime? deletedAt,
    bool clearDeletedAt = false,
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
      savedFilter:
          clearSavedFilter ? null : (savedFilter ?? this.savedFilter),
      synced: synced ?? this.synced,
      version: clearVersion ? null : (version ?? this.version),
      updatedAt: clearUpdatedAt ? null : (updatedAt ?? this.updatedAt),
      deletedAt: clearDeletedAt ? null : (deletedAt ?? this.deletedAt),
    );
  }

  factory Collection.fromLocalMap(Map<String, dynamic> map) {
    DateTime? parseOptional(dynamic raw) {
      if (raw is String && raw.isNotEmpty) return DateTime.parse(raw);
      return null;
    }

    return Collection(
      uuid: map['uuid'] as String,
      name: map['name'] as String,
      description: map['description'] as String?,
      colorHex: map['color_hex'] as String?,
      iconKey: map['icon_key'] as String?,
      createdAt: DateTime.parse(map['created_at'] as String),
      sortOrder: (map['sort_order'] as num?)?.toInt() ?? 0,
      savedFilter: SavedFilter.fromJsonString(map['saved_filter'] as String?),
      synced: map['synced'] == 1 || map['synced'] == true,
      version: (map['version'] as num?)?.toInt(),
      updatedAt: parseOptional(map['updated_at']),
      deletedAt: parseOptional(map['deleted_at']),
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
      'saved_filter': savedFilter?.toJsonString(),
      'synced': synced ? 1 : 0,
      'version': version,
      'updated_at': updatedAt?.toIso8601String(),
      'deleted_at': deletedAt?.toIso8601String(),
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
          sortOrder == other.sortOrder &&
          savedFilter == other.savedFilter &&
          synced == other.synced &&
          version == other.version &&
          updatedAt == other.updatedAt &&
          deletedAt == other.deletedAt;

  @override
  int get hashCode => Object.hash(
        uuid,
        name,
        description,
        colorHex,
        iconKey,
        createdAt,
        sortOrder,
        savedFilter,
        synced,
        version,
        updatedAt,
        deletedAt,
      );
}
