import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/sharing_models.dart';
import 'sharing_api_client.dart';

/// In-memory cache of the user's effective role per resource. Populated
/// by polling /api/sharing/resources/sync and queried by
/// canEditProvider / shareMetaProvider on a per-resource basis.
///
/// The store has no persistence (yet) — when the app cold-starts and the
/// user is signed in, the first sync fills the cache; cache misses
/// fall back to "treat as owner" so a freshly-created local resource
/// isn't blocked before its envelope arrives on the next sync.
class RoleCacheRepository {
  final SharingApiClient _api;

  final Map<String, ResourceEnvelope> _byId = {};
  DateTime? _lastSyncedAt;

  RoleCacheRepository(this._api);

  ResourceEnvelope? envelopeFor(String resourceId) => _byId[resourceId];

  /// Returns the effective role, or null if this resource is unknown to
  /// the sharing service yet. Callers should treat null as "I'm the
  /// implicit owner of a locally-created resource not yet synced".
  EffectiveRole? roleFor(String resourceId) => _byId[resourceId]?.myRole;

  /// Pulls the delta from the server, applies it to the cache, and
  /// returns the affected ids (useful for listeners that want to
  /// invalidate dependent providers).
  Future<Set<String>> sync({List<String>? types}) async {
    final page = await _api.syncResources(since: _lastSyncedAt, types: types);
    final affected = <String>{};
    for (final envelope in page.items) {
      affected.add(envelope.id);
      if (envelope.deleted) {
        _byId.remove(envelope.id);
      } else {
        _byId[envelope.id] = envelope;
      }
    }
    _lastSyncedAt = page.serverTime;
    return affected;
  }

  /// Clears the cache. Called on sign-out so the next signed-in
  /// session doesn't see another user's resources.
  void reset() {
    _byId.clear();
    _lastSyncedAt = null;
  }
}

final roleCacheRepositoryProvider = Provider<RoleCacheRepository>((ref) {
  return RoleCacheRepository(ref.watch(sharingApiClientProvider));
});
