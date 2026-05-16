# Report: `CacheApi` wrapper remnant and `api.dart` purity

**Status:** investigation complete, fix not started
**Audit date:** 2026-05-15
**Branch this report lives on:** `claude/api-purity-report`
**Continues:** PR #61 (Riverpod unification, dropped `OfflineApi`/`MapApi`) and PR #62 (api.dart boundary restoration)

## Why this report exists

PR #61's stated goal was to "drop the wrapper class pattern entirely" — `OfflineApi` and `MapApi` were deleted because they were pure pass-through classes around their notifiers. The architecture doc was updated to say:

> The notifier IS the public API. Consumers call `ref.read(xxxProvider.notifier).method(...)` directly. Do not introduce wrapper classes — they add indirection without buying testability (notifiers are equally mockable via `ProviderContainer.override`).

Two violations of that rule survived the migration:

1. **`cached_tiles/api.dart` still contains a `CacheApi` wrapper class** — the same shape as the deleted `OfflineApi`/`MapApi`, just with a different name. 32 lines of pure indirection.
2. **`tile_providers/api.dart` defines providers inline in `api.dart`** instead of in `data/`. The doc says provider globals "live in the same `data/` file as their notifier, and are re-exported from `api.dart` via `export ... show`." This file currently re-exports nothing meaningful for providers — it just declares them in place.

These are continuations of the cleanup work, not new architectural concerns. Both are mechanical fixes; together they remove the last reasons any feature's `api.dart` needs to import anything other than re-exports.

## Finding 1 — `CacheApi` wrapper class

**File:** `lib/features/tile_storage/cached_tiles/api.dart` (42 lines, ~22 of which are code, not exports)

**The class:**

```dart
final cacheApiProvider = FutureProvider<CacheApi>((ref) async {
  final cacheService = await ref.watch(cacheServiceProvider.future);
  return CacheApi(cacheService: cacheService);
});

class CacheApi {
  final CacheService _cacheService;
  CacheApi({required CacheService cacheService}) : _cacheService = cacheService;

  TileProvider createTileProvider({
    required String urlTemplate,
    Map<String, String>? headers,
    String? userAgentPackageName,
  }) {
    return _cacheService.createTileProvider(
      urlTemplate: urlTemplate,
      headers: headers,
      userAgentPackageName: userAgentPackageName,
    );
  }

  Future<int> clearCache() {
    return _cacheService.clear();
  }
}
```

Every method is a one-line pass-through to `CacheService` (`cache_service.dart:46`). The only renames are `clear()` → `clearCache()`. No state, no caching, no enrichment.

**Underlying class is already a full feature:** `CacheService` (`cache_service.dart:46-…`) owns `tileStore`, the `Dio` client, in-flight request deduplication, and `createTileProvider` itself. It is the real "API of this feature."

**Providers:**
- `cacheServiceProvider` (internal, `data/cache_service.dart:26`) — `FutureProvider<CacheService>` that wires `TileStoreService` + `Dio`.
- `cacheApiProvider` (public, `api.dart:11`) — `FutureProvider<CacheApi>` that wraps the above.

The wrapper exists only because the file comment calls `cacheServiceProvider` "internal." But after PR #61's reasoning ("notifier is the public API"), the analog here is: **service is the public API**. The wrapper buys nothing.

**Callsites of `cacheApiProvider` (2):**

| File | Line | Use |
|---|---|---|
| `lib/features/tile_providers/data/tile_registry.dart` | 200 | `ref.read(cacheApiProvider)` → `.when(data: (cacheApi) => cacheApi.createTileProvider(...))` |
| `test/features/tile_storage/cached_tiles/cached_tiles_test.dart` | 118 | `await container.read(cacheApiProvider.future)` then calls `cacheApi.createTileProvider(...)` |

Both callsites would change to `ref.read(cacheServiceProvider).whenOrNull(data: (svc) => svc.createTileProvider(...))` (and analog for the test). The behavior is identical; one indirection layer disappears.

**Fix:**

1. Delete the `CacheApi` class and `cacheApiProvider` from `cached_tiles/api.dart`.
2. Promote `cacheServiceProvider` from `data/cache_service.dart` to be the feature's public provider — re-export it (and `CacheService`) from `api.dart`. Remove the "internal" comment.
3. Update the 2 callsites.
4. If `clearCache()` (vs `clear()`) is a name the codebase wants to keep, rename `CacheService.clear()` → `CacheService.clearCache()` for consistency. (Otherwise let the rename die with the wrapper.)

**Impact:** 32 lines deleted from `api.dart`, 2 lines changed at each callsite, no behavior change. Code is more consistent with the rest of the codebase, and the architecture doc's rule about wrapper classes finally holds everywhere.

## Finding 2 — `tile_providers/api.dart` violates "thin façade"

**File:** `lib/features/tile_providers/api.dart` (59 lines, only 3 exports)

**Current shape:**

```dart
library;

import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/tile_providers/data/tile_registry.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/features/tile_providers/models/tile_registry_state.dart';

export 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
export 'package:turbo/features/tile_providers/models/tile_registry_state.dart';
export 'package:turbo/features/tile_providers/data/providers/osm_tiles.dart' show OsmConfig;

final tileRegistryProvider =
    NotifierProvider<TileRegistry, TileRegistryState>(TileRegistry.new);

final activeTileLayersProvider = Provider<List<TileLayer>>((ref) {
  ref.watch(tileRegistryProvider);
  return ref.read(tileRegistryProvider.notifier).getActiveLayers();
});

final globalLayersProvider = Provider<List<TileProviderConfig>>((ref) { ... });
final localLayersProvider  = Provider<List<TileProviderConfig>>((ref) { ... });
final overlayLayersProvider = Provider<List<TileProviderConfig>>((ref) { ... });
final offlineLayersProvider = Provider<List<TileProviderConfig>>((ref) { ... });
```

Six provider globals are declared here directly. Per the architecture doc's "Provider declaration location" rule (added in PR #61), all of them should live in a `data/` file and be re-exported.

**Why this matters beyond aesthetics:**
- Every other feature's `api.dart` is a pure façade. The boundary regression test (`test/architecture/feature_boundary_test.dart`, on PR #62) only checks that imports go through `api.dart` — it doesn't yet enforce that `api.dart` is export-only. This file is the only one that breaks the pattern, and any new feature contributor copying it as a template would propagate the violation.
- The `import 'package:flutter_map/...';` and `import 'package:flutter_riverpod/...';` lines at the top of `api.dart` are a smell: the public surface of a feature shouldn't need to import frameworks. Move the providers and those imports move with them.

**Callsites:** all 5 derived providers are used. `tileRegistryProvider` and `activeTileLayersProvider` are widely used (map page, region creation, offline regions page, map layer button, measuring map page, download orchestrator); the four category-filter providers (`globalLayersProvider`, `localLayersProvider`, `overlayLayersProvider`, `offlineLayersProvider`) are used only by `map_layer_button.dart`. Behavior must be preserved exactly.

**Fix:**

1. Move all 6 provider declarations into `data/tile_registry.dart` (or a sibling `data/tile_layer_providers.dart` if that file grows uncomfortably long — `tile_registry.dart` is already ~250 lines, so splitting is the better call).
2. In `api.dart`, replace the in-place declarations with re-exports:
   ```dart
   library;
   export 'data/tile_registry.dart' show tileRegistryProvider, TileRegistry;
   export 'data/tile_layer_providers.dart'
       show activeTileLayersProvider, globalLayersProvider,
            localLayersProvider, overlayLayersProvider, offlineLayersProvider;
   export 'models/tile_provider_config.dart';
   export 'models/tile_registry_state.dart';
   export 'data/providers/osm_tiles.dart' show OsmConfig;
   ```
3. Delete the now-unused `import 'package:flutter_map/...';` and `import 'package:flutter_riverpod/...';` from `api.dart`.
4. No callsite changes — all consumers already import `tile_providers/api.dart`, which keeps exporting the same symbols.

**Impact:** ~45 lines move from `api.dart` to `data/`, no behavior change, the file becomes a clean façade like every other feature.

## Suggested follow-up: strengthen the boundary test

After both fixes land, **every `api.dart` file in the codebase consists only of `library;` + `export` directives** (and possibly a doc comment). At that point it's worth tightening `test/architecture/feature_boundary_test.dart` (or adding a sibling test) to assert this invariant:

```dart
test('every features/<X>/api.dart is a pure re-export façade', () {
  // For each api.dart file, parse out top-level declarations.
  // Fail if any line is not blank, not a comment, not "library;",
  // and not an export directive.
});
```

This makes both findings impossible to regress and reframes the architecture rule as code instead of convention. The test is cheap to write (same `dart:io` walk as the existing boundary test).

This follow-up is **not part of the fix scope** for findings 1 and 2 — it should land in its own PR after the cleanup, so the test starts green from day one.

## Out of scope for this report

- `tile_providers/api.dart` references the comment "Convenience providers for filtering available layers by category for the UI." These derivations are useful and should be kept; the report is about where they LIVE, not whether they exist.
- The architecture doc itself doesn't need changes — both findings violate rules that are already written down.
- Renaming `CacheService.clear()` to `clearCache()` is mentioned as an option but does not block the fix; the wrapper rename was never load-bearing.

## Recommended PR shape

One PR titled something like **"Drop the last wrapper class and make every api.dart a pure façade."** Two commits:

1. `Drop CacheApi wrapper and expose cacheServiceProvider directly` — Finding 1.
2. `Move tile_providers providers into data/, make api.dart a façade` — Finding 2.

Test plan:
- `flutter analyze` — clean
- `flutter test` — all 128 tests pass (no test additions strictly needed; both fixes are pure refactors, and the existing `cached_tiles_test.dart` and `tile_registry`-using tests catch behavior changes)
- Grep gate: `grep -rn "CacheApi\|cacheApiProvider" lib/ test/` returns empty
- Manual smoke: open a map screen, verify cached and active tile layers render exactly as before

## Critical files referenced

- `lib/features/tile_storage/cached_tiles/api.dart` — Finding 1, to be cleaned
- `lib/features/tile_storage/cached_tiles/data/cache_service.dart` — promoting `cacheServiceProvider`
- `lib/features/tile_providers/data/tile_registry.dart:200` — `cacheApiProvider` callsite
- `test/features/tile_storage/cached_tiles/cached_tiles_test.dart:118` — `cacheApiProvider` callsite
- `lib/features/tile_providers/api.dart` — Finding 2, to be cleaned
- `lib/features/tile_providers/data/tile_registry.dart` — destination for `tileRegistryProvider`
- `lib/context/architecture.context.md` — the rules these findings flag against
- `test/architecture/feature_boundary_test.dart` (on PR #62) — could be extended with the façade-purity check as follow-up
