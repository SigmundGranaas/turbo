import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/role_cache_repository.dart';
import '../data/sharing_api_client.dart';
import '../models/sharing_models.dart';

/// Whether the identity-aware sharing UI should be visible. Defaults to
/// false so widgets that gate on this provider don't accidentally
/// instantiate the full authStateProvider in their dependency tree
/// (which schedules background init timers that leak in widget tests).
///
/// The app overrides this in main() so it tracks auth status in
/// production. Tests that exercise the sharing UI directly can override
/// to true with a mocked auth context.
final sharingAvailableProvider = Provider<bool>((ref) => false);

/// True iff the user can edit the given resource. Locally-created
/// resources not yet known to Sharing fall through as editable
/// (implicit owner) so the offline-first path is unaffected. Anonymous
/// users always pass.
final canEditProvider = Provider.family<bool, String>((ref, resourceId) {
  if (!ref.watch(sharingAvailableProvider)) return true;
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
  if (!ref.watch(sharingAvailableProvider)) return const [];
  final api = ref.watch(sharingApiClientProvider);
  return api.listFriendships(status: FriendshipStatus.accepted);
});

/// All friendships including pending and blocked — used by the
/// FriendsPage's tabs.
final allFriendshipsProvider = FutureProvider<List<Friendship>>((ref) async {
  if (!ref.watch(sharingAvailableProvider)) return const [];
  final api = ref.watch(sharingApiClientProvider);
  return api.listFriendships();
});

final myGroupsProvider = FutureProvider<List<FriendGroup>>((ref) async {
  if (!ref.watch(sharingAvailableProvider)) return const [];
  final api = ref.watch(sharingApiClientProvider);
  return api.listGroups();
});

/// Grants currently attached to a resource. Owner-only; the API
/// rejects non-owners with 403. UI should gate this provider on
/// `effectiveRoleProvider == EffectiveRole.owner`.
final grantsForResourceProvider =
    FutureProvider.family<List<Grant>, String>((ref, resourceId) async {
  if (!ref.watch(sharingAvailableProvider)) return const [];
  final api = ref.watch(sharingApiClientProvider);
  return api.listGrantsForResource(resourceId);
});
