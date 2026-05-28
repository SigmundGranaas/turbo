import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/api/api_client.dart';
import 'package:turbo/features/sharing/api.dart';

/// A SharingApiClient stand-in for widget tests. Subclasses the real
/// class and overrides the network methods the tests exercise. Tracks
/// the calls and returns canned responses; the ApiClient passed to the
/// super constructor is never used (no overridden method calls super).
class FakeSharingApi extends SharingApiClient {
  FakeSharingApi() : super(ApiClient());

  // ── Test state ──────────────────────────────────────────────────────
  List<Friendship> acceptedFriends = const [];
  List<Friendship> allFriendships = const [];
  List<FriendGroup> groups = const [];
  List<Grant> grants = const [];
  UserProfile? myProfile;

  // ── Recorded calls ──────────────────────────────────────────────────
  final List<({String resourceId, String userId, GrantRole role})> userGrants = [];
  final List<({String resourceId, String groupId, GrantRole role})> groupGrants = [];
  final List<({String resourceId, GrantRole role})> linkGrants = [];
  final List<({String resourceId, String subjectId, String subjectType})> revocations = [];
  final List<String> friendshipRequests = [];
  final List<String> friendshipAccepts = [];
  final List<String> friendshipRemoves = [];
  final List<({String email, int? lifetimeDays})> friendInvites = [];
  final List<String> friendCodeLookups = [];
  final List<({String name})> groupsCreated = [];

  // ── Reads ───────────────────────────────────────────────────────────

  /// Last sync call's `since` cursor + types filter. Tests can assert
  /// on what the listener requested.
  ({DateTime? since, List<String>? types})? lastSync;

  @override
  Future<ResourceSyncPage> syncResources({
    DateTime? since,
    List<String>? types,
    int? limit,
  }) async {
    lastSync = (since: since, types: types);
    return ResourceSyncPage(items: const [], serverTime: DateTime.utc(2026, 1, 1));
  }

  @override
  Future<List<Friendship>> listFriendships({FriendshipStatus? status}) async {
    if (status == FriendshipStatus.accepted) return acceptedFriends;
    return allFriendships;
  }

  @override
  Future<List<FriendGroup>> listGroups() async => groups;

  @override
  Future<FriendGroup?> getGroup(String groupId) async =>
      groups.where((g) => g.id == groupId).firstOrNull;

  @override
  Future<List<Grant>> listGrantsForResource(String resourceId) async => grants;

  @override
  Future<UserProfile> getMyProfile() async =>
      myProfile ?? (throw StateError('myProfile not seeded'));

  @override
  Future<String?> lookupUserByFriendCode(String code) async {
    friendCodeLookups.add(code);
    // Default: pretend any non-empty code resolves to a deterministic id.
    return code.trim().isEmpty ? null : '11111111-1111-1111-1111-111111111111';
  }

  // ── Writes ──────────────────────────────────────────────────────────
  @override
  Future<Grant> grantToUser(String resourceId, String userId, GrantRole role,
      {DateTime? expiresAt}) async {
    userGrants.add((resourceId: resourceId, userId: userId, role: role));
    final g = Grant(
      resourceId: resourceId,
      subjectType: 'user',
      subjectId: userId,
      role: role,
      grantedBy: 'me',
      grantedAt: DateTime.utc(2026, 1, 1),
      expiresAt: null,
    );
    grants = [...grants, g];
    return g;
  }

  @override
  Future<Grant> grantToGroup(String resourceId, String groupId, GrantRole role,
      {DateTime? expiresAt}) async {
    groupGrants.add((resourceId: resourceId, groupId: groupId, role: role));
    final g = Grant(
      resourceId: resourceId,
      subjectType: 'group',
      subjectId: groupId,
      role: role,
      grantedBy: 'me',
      grantedAt: DateTime.utc(2026, 1, 1),
      expiresAt: null,
    );
    grants = [...grants, g];
    return g;
  }

  @override
  Future<LinkGrant> grantAsLink(String resourceId, GrantRole role,
      {DateTime? expiresAt}) async {
    linkGrants.add((resourceId: resourceId, role: role));
    return LinkGrant(
      resourceId: resourceId,
      subjectId: 'subj',
      linkToken: 'tok-${linkGrants.length}',
      role: role,
      grantedAt: DateTime.utc(2026, 1, 1),
      expiresAt: null,
    );
  }

  @override
  Future<void> revokeUserGrant(String resourceId, String userId) async {
    revocations.add((resourceId: resourceId, subjectId: userId, subjectType: 'user'));
    grants = grants.where((g) => !(g.subjectType == 'user' && g.subjectId == userId)).toList();
  }

  @override
  Future<void> revokeGroupGrant(String resourceId, String groupId) async {
    revocations.add((resourceId: resourceId, subjectId: groupId, subjectType: 'group'));
    grants = grants.where((g) => !(g.subjectType == 'group' && g.subjectId == groupId)).toList();
  }

  @override
  Future<void> revokeLinkGrant(String resourceId, String linkSubjectId) async {
    revocations.add((resourceId: resourceId, subjectId: linkSubjectId, subjectType: 'link'));
    grants = grants.where((g) => !(g.subjectType == 'link' && g.subjectId == linkSubjectId)).toList();
  }

  @override
  Future<Friendship> requestFriendship(String otherUserId) async {
    friendshipRequests.add(otherUserId);
    return Friendship(
      otherUserId: otherUserId,
      initiatorId: 'me',
      status: FriendshipStatus.pending,
      createdAt: DateTime.utc(2026, 1, 1),
      acceptedAt: null,
    );
  }

  @override
  Future<Friendship> acceptFriendship(String otherUserId) async {
    friendshipAccepts.add(otherUserId);
    return Friendship(
      otherUserId: otherUserId,
      initiatorId: otherUserId,
      status: FriendshipStatus.accepted,
      createdAt: DateTime.utc(2026, 1, 1),
      acceptedAt: DateTime.utc(2026, 1, 2),
    );
  }

  @override
  Future<void> removeFriendship(String otherUserId) async {
    friendshipRemoves.add(otherUserId);
  }

  @override
  Future<ShareInvite> createFriendInvite(String email, {int? lifetimeDays}) async {
    friendInvites.add((email: email, lifetimeDays: lifetimeDays));
    return ShareInvite(
      id: 'invite-${friendInvites.length}',
      inviterId: 'me',
      inviteeEmail: email,
      resourceId: null,
      role: null,
      createdAt: DateTime.utc(2026, 1, 1),
      expiresAt: null,
    );
  }

  @override
  Future<FriendGroup> createGroup(String name) async {
    groupsCreated.add((name: name));
    final g = FriendGroup(
      id: 'group-${groupsCreated.length}',
      ownerId: 'me',
      name: name,
      createdAt: DateTime.utc(2026, 1, 1),
      updatedAt: DateTime.utc(2026, 1, 1),
      members: [
        GroupMemberInfo(userId: 'me', role: 'admin', joinedAt: DateTime.utc(2026, 1, 1)),
      ],
    );
    groups = [...groups, g];
    return g;
  }
}

/// Standard test wrapper. Overrides:
/// - sharingAvailableProvider → true (so UI is visible and FutureProviders fire)
/// - sharingApiClientProvider → the supplied FakeSharingApi
Widget wrapWithSharingFake(Widget child, FakeSharingApi api) {
  return ProviderScope(
    overrides: [
      sharingAvailableProvider.overrideWith((ref) => true),
      sharingApiClientProvider.overrideWith((ref) => api),
    ],
    child: MaterialApp(
      localizationsDelegates: const [
        AppLocalizations.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: AppLocalizations.supportedLocales,
      home: child is Scaffold ? child : Scaffold(body: child),
    ),
  );
}
