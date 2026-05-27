import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/auth/data/auth_providers.dart';

import '../data/role_cache_repository.dart';
import '../data/sharing_api_client.dart';
import '../models/sharing_models.dart';

/// True iff the user can edit the given resource. Locally-created
/// resources not yet known to Sharing fall through as editable
/// (implicit owner) so the offline-first path is unaffected. Anonymous
/// users always pass.
final canEditProvider = Provider.family<bool, String>((ref, resourceId) {
  final auth = ref.watch(authStateProvider);
  if (auth.status != AuthStatus.authenticated) return true;
  final role = ref.watch(roleCacheRepositoryProvider).roleFor(resourceId);
  return role == null || role.canEdit;
});

/// The user's effective role on a resource, or null if Sharing has no
/// knowledge of it yet (treat as owner of a locally-created resource).
final effectiveRoleProvider =
    Provider.family<EffectiveRole?, String>((ref, resourceId) {
  return ref.watch(roleCacheRepositoryProvider).roleFor(resourceId);
});

/// The friend list, filtered to accepted friendships. Refresh by
/// reading `ref.refresh(acceptedFriendsProvider)`.
final acceptedFriendsProvider = FutureProvider<List<Friendship>>((ref) async {
  final auth = ref.watch(authStateProvider);
  if (auth.status != AuthStatus.authenticated) return const [];
  final api = ref.watch(sharingApiClientProvider);
  return api.listFriendships(status: FriendshipStatus.accepted);
});

/// All friendships including pending and blocked — used by the
/// FriendsPage's tabs.
final allFriendshipsProvider = FutureProvider<List<Friendship>>((ref) async {
  final auth = ref.watch(authStateProvider);
  if (auth.status != AuthStatus.authenticated) return const [];
  final api = ref.watch(sharingApiClientProvider);
  return api.listFriendships();
});

final myGroupsProvider = FutureProvider<List<FriendGroup>>((ref) async {
  final auth = ref.watch(authStateProvider);
  if (auth.status != AuthStatus.authenticated) return const [];
  final api = ref.watch(sharingApiClientProvider);
  return api.listGroups();
});

/// Grants currently attached to a resource. Owner-only; the API
/// rejects non-owners with 403. UI should gate this provider on
/// `effectiveRoleProvider == EffectiveRole.owner`.
final grantsForResourceProvider =
    FutureProvider.family<List<Grant>, String>((ref, resourceId) async {
  final auth = ref.watch(authStateProvider);
  if (auth.status != AuthStatus.authenticated) return const [];
  final api = ref.watch(sharingApiClientProvider);
  return api.listGrantsForResource(resourceId);
});
