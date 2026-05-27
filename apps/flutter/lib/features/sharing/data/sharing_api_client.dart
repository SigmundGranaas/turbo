import 'package:dio/dio.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/core/api/api_client.dart';
import 'package:turbo/features/auth/api.dart';

import '../models/sharing_models.dart';

/// HTTP wrapper for the /api/sharing/* surface. Stateless; all methods
/// return raw DTOs and let the Riverpod providers above own caching.
class SharingApiClient {
  final ApiClient _api;

  SharingApiClient(this._api);

  static const _friendshipsBase = '/api/sharing/friendships';
  static const _groupsBase = '/api/sharing/groups';
  static const _grantsBase = '/api/sharing/grants';
  static const _invitesBase = '/api/sharing/invites';
  static const _resourcesBase = '/api/sharing/resources';

  static Options _json({Map<String, dynamic>? headers}) => Options(
        headers: {
          'Content-Type': 'application/json',
          if (headers != null) ...headers,
        },
        validateStatus: (s) => s != null && s < 500,
      );

  // ── Friendships ──────────────────────────────────────────────────────
  Future<List<Friendship>> listFriendships({FriendshipStatus? status}) async {
    final query = status != null
        ? {'status': friendshipStatusToWire(status)}
        : <String, dynamic>{};
    final r = await _api.get(_friendshipsBase, queryParameters: query);
    _ensureOk(r);
    return (r.data as List)
        .map((e) => Friendship.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  Future<Friendship> requestFriendship(String otherUserId) async {
    final r = await _api.post('$_friendshipsBase/request',
        data: {'otherUserId': otherUserId}, options: _json());
    _ensureOk(r);
    return Friendship.fromJson(r.data as Map<String, dynamic>);
  }

  Future<Friendship> acceptFriendship(String otherUserId) async {
    final r = await _api.post('$_friendshipsBase/accept',
        data: {'otherUserId': otherUserId}, options: _json());
    _ensureOk(r);
    return Friendship.fromJson(r.data as Map<String, dynamic>);
  }

  Future<void> blockFriendship(String otherUserId) async {
    final r = await _api.post('$_friendshipsBase/block',
        data: {'otherUserId': otherUserId}, options: _json());
    _ensureNoContent(r);
  }

  Future<void> removeFriendship(String otherUserId) async {
    final r = await _api.delete('$_friendshipsBase/$otherUserId');
    _ensureNoContent(r);
  }

  // ── Groups ───────────────────────────────────────────────────────────
  Future<List<FriendGroup>> listGroups() async {
    final r = await _api.get(_groupsBase);
    _ensureOk(r);
    return (r.data as List)
        .map((e) => FriendGroup.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  Future<FriendGroup> createGroup(String name) async {
    final r = await _api.post(_groupsBase, data: {'name': name}, options: _json());
    if (r.statusCode != 201 && r.statusCode != 200) {
      throw _err('createGroup', r);
    }
    return FriendGroup.fromJson(r.data as Map<String, dynamic>);
  }

  Future<FriendGroup?> getGroup(String groupId) async {
    final r = await _api.get('$_groupsBase/$groupId');
    if (r.statusCode == 404) return null;
    _ensureOk(r);
    return FriendGroup.fromJson(r.data as Map<String, dynamic>);
  }

  Future<void> deleteGroup(String groupId) async {
    final r = await _api.delete('$_groupsBase/$groupId');
    _ensureNoContent(r);
  }

  Future<void> renameGroup(String groupId, String name) async {
    final r = await _api.put('$_groupsBase/$groupId/name',
        data: {'name': name}, options: _json());
    _ensureNoContent(r);
  }

  Future<void> addGroupMember(String groupId, String userId) async {
    final r = await _api.post('$_groupsBase/$groupId/members',
        data: {'userId': userId}, options: _json());
    _ensureNoContent(r);
  }

  Future<void> removeGroupMember(String groupId, String userId) async {
    final r = await _api.delete('$_groupsBase/$groupId/members/$userId');
    _ensureNoContent(r);
  }

  // ── Grants ───────────────────────────────────────────────────────────
  Future<List<Grant>> listGrantsForResource(String resourceId) async {
    final r = await _api.get('$_grantsBase/resources/$resourceId');
    _ensureOk(r);
    return (r.data as List)
        .map((e) => Grant.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  Future<Grant> grantToUser(String resourceId, String userId, GrantRole role,
      {DateTime? expiresAt}) async {
    final r = await _api.post('$_grantsBase/users',
        data: {
          'resourceId': resourceId,
          'userId': userId,
          'role': grantRoleToWire(role),
          if (expiresAt != null) 'expiresAt': expiresAt.toUtc().toIso8601String(),
        },
        options: _json());
    _ensureOk(r);
    return Grant.fromJson(r.data as Map<String, dynamic>);
  }

  Future<Grant> grantToGroup(String resourceId, String groupId, GrantRole role,
      {DateTime? expiresAt}) async {
    final r = await _api.post('$_grantsBase/groups',
        data: {
          'resourceId': resourceId,
          'groupId': groupId,
          'role': grantRoleToWire(role),
          if (expiresAt != null) 'expiresAt': expiresAt.toUtc().toIso8601String(),
        },
        options: _json());
    _ensureOk(r);
    return Grant.fromJson(r.data as Map<String, dynamic>);
  }

  Future<LinkGrant> grantAsLink(String resourceId, GrantRole role,
      {DateTime? expiresAt}) async {
    final r = await _api.post('$_grantsBase/links',
        data: {
          'resourceId': resourceId,
          'role': grantRoleToWire(role),
          if (expiresAt != null) 'expiresAt': expiresAt.toUtc().toIso8601String(),
        },
        options: _json());
    _ensureOk(r);
    return LinkGrant.fromJson(r.data as Map<String, dynamic>);
  }

  Future<void> revokeUserGrant(String resourceId, String userId) async {
    final r = await _api.delete('$_grantsBase/resources/$resourceId/users/$userId');
    _ensureNoContent(r);
  }

  Future<void> revokeGroupGrant(String resourceId, String groupId) async {
    final r = await _api.delete('$_grantsBase/resources/$resourceId/groups/$groupId');
    _ensureNoContent(r);
  }

  Future<void> revokeLinkGrant(String resourceId, String linkSubjectId) async {
    final r = await _api.delete('$_grantsBase/resources/$resourceId/links/$linkSubjectId');
    _ensureNoContent(r);
  }

  /// Redeems a link token as the calling user. Server materializes a
  /// user grant on the same resource so it shows up in normal sync.
  Future<LinkRedemption> redeemLink(String token) async {
    final r = await _api.post('$_grantsBase/links/$token/redeem', options: _json());
    _ensureOk(r);
    return LinkRedemption.fromJson(r.data as Map<String, dynamic>);
  }

  // ── Invites ──────────────────────────────────────────────────────────
  Future<List<ShareInvite>> listMyInvites() async {
    final r = await _api.get(_invitesBase);
    _ensureOk(r);
    return (r.data as List)
        .map((e) => ShareInvite.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  Future<ShareInvite> createFriendInvite(String email, {int? lifetimeDays}) async {
    final r = await _api.post('$_invitesBase/friend',
        data: {'email': email, if (lifetimeDays != null) 'lifetimeDays': lifetimeDays},
        options: _json());
    _ensureOk(r);
    return ShareInvite.fromJson(r.data as Map<String, dynamic>);
  }

  Future<ShareInvite> createResourceInvite(
      String email, String resourceId, GrantRole role,
      {int? lifetimeDays}) async {
    final r = await _api.post('$_invitesBase/resource',
        data: {
          'email': email,
          'resourceId': resourceId,
          'role': grantRoleToWire(role),
          if (lifetimeDays != null) 'lifetimeDays': lifetimeDays,
        },
        options: _json());
    _ensureOk(r);
    return ShareInvite.fromJson(r.data as Map<String, dynamic>);
  }

  Future<int> redeemInvites(String email) async {
    final r = await _api.post('$_invitesBase/redeem',
        data: {'email': email}, options: _json());
    _ensureOk(r);
    final data = r.data as Map<String, dynamic>;
    return (data['redeemed'] as num).toInt();
  }

  // ── Resources sync ───────────────────────────────────────────────────
  Future<ResourceSyncPage> syncResources({
    DateTime? since,
    List<String>? types,
    int? limit,
  }) async {
    final query = <String, dynamic>{
      if (since != null) 'since': since.toUtc().toIso8601String(),
      if (types != null && types.isNotEmpty) 'types': types.join(','),
      if (limit != null) 'limit': limit,
    };
    final r = await _api.get('$_resourcesBase/sync', queryParameters: query);
    _ensureOk(r);
    return ResourceSyncPage.fromJson(r.data as Map<String, dynamic>);
  }

  void _ensureOk(Response r) {
    if (r.statusCode == null || r.statusCode! >= 400) throw _err('request', r);
  }

  void _ensureNoContent(Response r) {
    if (r.statusCode != 204) throw _err('request', r);
  }

  Exception _err(String op, Response r) =>
      Exception('Sharing $op failed: ${r.statusCode} ${r.data}');
}

final sharingApiClientProvider = Provider<SharingApiClient>((ref) {
  return SharingApiClient(ref.watch(authenticatedApiClientProvider));
});
