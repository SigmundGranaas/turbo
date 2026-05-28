/// Domain models for the Sharing feature. Mirrors the wire DTOs the
/// /api/sharing/* endpoints return. Kept value-only (no providers, no
/// repository methods) so the data layer can pass these around freely.
library;

/// Visibility levels a resource can be in.
enum ResourceVisibility { private, friends, unlistedLink, public }

ResourceVisibility resourceVisibilityFromWire(String wire) => switch (wire) {
      'private' => ResourceVisibility.private,
      'friends' => ResourceVisibility.friends,
      'unlisted_link' => ResourceVisibility.unlistedLink,
      'public' => ResourceVisibility.public,
      _ => throw ArgumentError('Unknown visibility: $wire'),
    };

String resourceVisibilityToWire(ResourceVisibility v) => switch (v) {
      ResourceVisibility.private => 'private',
      ResourceVisibility.friends => 'friends',
      ResourceVisibility.unlistedLink => 'unlisted_link',
      ResourceVisibility.public => 'public',
    };

/// The role a user effectively holds on a resource.
enum EffectiveRole { viewer, editor, owner }

EffectiveRole effectiveRoleFromWire(String wire) => switch (wire) {
      'viewer' => EffectiveRole.viewer,
      'editor' => EffectiveRole.editor,
      'owner' => EffectiveRole.owner,
      _ => throw ArgumentError('Unknown role: $wire'),
    };

extension EffectiveRoleExt on EffectiveRole {
  bool get canEdit => this == EffectiveRole.editor || this == EffectiveRole.owner;
  bool get isOwner => this == EffectiveRole.owner;
}

/// Grant role assigned by a user grant / group grant / link grant.
enum GrantRole { viewer, editor }

String grantRoleToWire(GrantRole r) => switch (r) {
      GrantRole.viewer => 'viewer',
      GrantRole.editor => 'editor',
    };

GrantRole grantRoleFromWire(String wire) => switch (wire) {
      'viewer' => GrantRole.viewer,
      'editor' => GrantRole.editor,
      _ => throw ArgumentError('Unknown grant role: $wire'),
    };

/// The envelope returned by /api/sharing/resources/sync. Carries the
/// ownership/role metadata without the payload — clients fetch typed
/// bodies from the payload module's existing GET-by-id endpoint.
class ResourceEnvelope {
  final String id;
  final String type;
  final String ownerId;
  final ResourceVisibility visibility;
  final EffectiveRole myRole;
  final int version;
  final DateTime updatedAt;
  final bool deleted;

  const ResourceEnvelope({
    required this.id,
    required this.type,
    required this.ownerId,
    required this.visibility,
    required this.myRole,
    required this.version,
    required this.updatedAt,
    required this.deleted,
  });

  factory ResourceEnvelope.fromJson(Map<String, dynamic> json) => ResourceEnvelope(
        id: json['id'] as String,
        type: json['type'] as String,
        ownerId: json['ownerId'] as String,
        visibility: resourceVisibilityFromWire(json['visibility'] as String),
        myRole: effectiveRoleFromWire(json['myRole'] as String),
        version: (json['version'] as num).toInt(),
        updatedAt: DateTime.parse(json['updatedAt'] as String),
        deleted: json['deleted'] as bool? ?? false,
      );
}

class ResourceSyncPage {
  final List<ResourceEnvelope> items;
  final DateTime serverTime;
  const ResourceSyncPage({required this.items, required this.serverTime});

  factory ResourceSyncPage.fromJson(Map<String, dynamic> json) => ResourceSyncPage(
        items: (json['items'] as List)
            .map((e) => ResourceEnvelope.fromJson(e as Map<String, dynamic>))
            .toList(growable: false),
        serverTime: DateTime.parse(json['serverTime'] as String),
      );
}

/// Friendship statuses on the wire.
enum FriendshipStatus { pending, accepted, blocked }

String friendshipStatusToWire(FriendshipStatus s) => switch (s) {
      FriendshipStatus.pending => 'pending',
      FriendshipStatus.accepted => 'accepted',
      FriendshipStatus.blocked => 'blocked',
    };

FriendshipStatus friendshipStatusFromWire(String wire) => switch (wire) {
      'pending' => FriendshipStatus.pending,
      'accepted' => FriendshipStatus.accepted,
      'blocked' => FriendshipStatus.blocked,
      _ => throw ArgumentError('Unknown friendship status: $wire'),
    };

class Friendship {
  final String otherUserId;
  final String initiatorId;
  final FriendshipStatus status;
  final DateTime createdAt;
  final DateTime? acceptedAt;
  const Friendship({
    required this.otherUserId,
    required this.initiatorId,
    required this.status,
    required this.createdAt,
    required this.acceptedAt,
  });

  factory Friendship.fromJson(Map<String, dynamic> json) => Friendship(
        otherUserId: json['otherUserId'] as String,
        initiatorId: json['initiatorId'] as String,
        status: friendshipStatusFromWire(json['status'] as String),
        createdAt: DateTime.parse(json['createdAt'] as String),
        acceptedAt: json['acceptedAt'] != null
            ? DateTime.parse(json['acceptedAt'] as String)
            : null,
      );
}

class GroupMemberInfo {
  final String userId;
  final String role; // 'admin' | 'member'
  final DateTime joinedAt;
  const GroupMemberInfo({required this.userId, required this.role, required this.joinedAt});

  factory GroupMemberInfo.fromJson(Map<String, dynamic> json) => GroupMemberInfo(
        userId: json['userId'] as String,
        role: json['role'] as String,
        joinedAt: DateTime.parse(json['joinedAt'] as String),
      );
}

class FriendGroup {
  final String id;
  final String ownerId;
  final String name;
  final DateTime createdAt;
  final DateTime updatedAt;
  final List<GroupMemberInfo> members;
  const FriendGroup({
    required this.id,
    required this.ownerId,
    required this.name,
    required this.createdAt,
    required this.updatedAt,
    required this.members,
  });

  factory FriendGroup.fromJson(Map<String, dynamic> json) => FriendGroup(
        id: json['id'] as String,
        ownerId: json['ownerId'] as String,
        name: json['name'] as String,
        createdAt: DateTime.parse(json['createdAt'] as String),
        updatedAt: DateTime.parse(json['updatedAt'] as String),
        members: (json['members'] as List)
            .map((e) => GroupMemberInfo.fromJson(e as Map<String, dynamic>))
            .toList(growable: false),
      );
}

class Grant {
  final String resourceId;
  final String subjectType; // 'user' | 'group' | 'link'
  final String subjectId;
  final GrantRole role;
  final String grantedBy;
  final DateTime grantedAt;
  final DateTime? expiresAt;
  const Grant({
    required this.resourceId,
    required this.subjectType,
    required this.subjectId,
    required this.role,
    required this.grantedBy,
    required this.grantedAt,
    required this.expiresAt,
  });

  factory Grant.fromJson(Map<String, dynamic> json) => Grant(
        resourceId: json['resourceId'] as String,
        subjectType: json['subjectType'] as String,
        subjectId: json['subjectId'] as String,
        role: grantRoleFromWire(json['role'] as String),
        grantedBy: json['grantedBy'] as String,
        grantedAt: DateTime.parse(json['grantedAt'] as String),
        expiresAt: json['expiresAt'] != null
            ? DateTime.parse(json['expiresAt'] as String)
            : null,
      );
}

class LinkGrant {
  final String resourceId;
  final String subjectId;
  final String linkToken;
  final GrantRole role;
  final DateTime grantedAt;
  final DateTime? expiresAt;
  const LinkGrant({
    required this.resourceId,
    required this.subjectId,
    required this.linkToken,
    required this.role,
    required this.grantedAt,
    required this.expiresAt,
  });

  factory LinkGrant.fromJson(Map<String, dynamic> json) => LinkGrant(
        resourceId: json['resourceId'] as String,
        subjectId: json['subjectId'] as String,
        linkToken: json['linkToken'] as String,
        role: grantRoleFromWire(json['role'] as String),
        grantedAt: DateTime.parse(json['grantedAt'] as String),
        expiresAt: json['expiresAt'] != null
            ? DateTime.parse(json['expiresAt'] as String)
            : null,
      );
}

/// Result of POST /api/sharing/grants/links/{token}/redeem. The role
/// is the effective role on the redeemed resource: "viewer", "editor",
/// or "owner" (if the redeemer is the owner — no grant created).
class LinkRedemption {
  final String resourceId;
  final String resourceType;
  final String role;
  const LinkRedemption({
    required this.resourceId,
    required this.resourceType,
    required this.role,
  });

  factory LinkRedemption.fromJson(Map<String, dynamic> json) => LinkRedemption(
        resourceId: json['resourceId'] as String,
        resourceType: json['resourceType'] as String,
        role: json['role'] as String,
      );
}

/// The calling user's sharing identity. The `friendCode` is the
/// shareable identifier friends use to find each other.
class UserProfile {
  final String userId;
  final String friendCode;
  final DateTime createdAt;
  const UserProfile({
    required this.userId,
    required this.friendCode,
    required this.createdAt,
  });

  factory UserProfile.fromJson(Map<String, dynamic> json) => UserProfile(
        userId: json['userId'] as String,
        friendCode: json['friendCode'] as String,
        createdAt: DateTime.parse(json['createdAt'] as String),
      );

  /// The friend-code formatted for display, including the "turbo-" prefix
  /// readers expect when seeing it out of context.
  String get displayCode => 'turbo-$friendCode';
}

class ShareInvite {
  final String id;
  final String inviterId;
  final String inviteeEmail;
  final String? resourceId;
  final GrantRole? role;
  final DateTime createdAt;
  final DateTime? expiresAt;
  const ShareInvite({
    required this.id,
    required this.inviterId,
    required this.inviteeEmail,
    required this.resourceId,
    required this.role,
    required this.createdAt,
    required this.expiresAt,
  });

  factory ShareInvite.fromJson(Map<String, dynamic> json) => ShareInvite(
        id: json['id'] as String,
        inviterId: json['inviterId'] as String,
        inviteeEmail: json['inviteeEmail'] as String,
        resourceId: json['resourceId'] as String?,
        role: json['role'] != null ? grantRoleFromWire(json['role'] as String) : null,
        createdAt: DateTime.parse(json['createdAt'] as String),
        expiresAt: json['expiresAt'] != null
            ? DateTime.parse(json['expiresAt'] as String)
            : null,
      );
}
