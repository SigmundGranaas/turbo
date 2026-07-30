#!/usr/bin/env bash
# Architectural fitness function for the routing-engine layering.
#
# The module design (docs/architecture/2026-07-routing-engine-module-design.md
# §12) lists boundary invariants. A boundary that is only written down is a
# boundary that erodes — the whole reason this refactor exists is that
# `Arc<Dem>` reached twelve places inside the engine without anyone
# deciding it should. So the invariants are checked mechanically, and the
# check runs in CI next to the routing gate.
#
# Every rule below fails LOUD with the reason, not just an exit code.
#
#   ./tools/boundary_check.sh
#
# Exit: 0 all invariants hold, 1 a boundary was crossed.
set -uo pipefail
cd "$(dirname "$0")/.."

fail=0
note() { printf '  %s\n' "$*"; }
bad() { printf 'VIOLATION  %s\n' "$1"; shift; for l in "$@"; do note "$l"; done; fail=1; }
ok() { printf 'ok         %s\n' "$1"; }

# ---------------------------------------------------------------------
# 1. The model crate has zero dependencies.
#
# This is the enforcement mechanism for "the engine never acquires
# anything": a port that cannot name a file cannot open one. `serde` is
# the single allowed exception (domain types are serialised at the API
# edge), and it is not currently needed.
# ---------------------------------------------------------------------
deps=$(awk '/^\[dependencies\]/{f=1;next} /^\[/{f=0} f && NF && $0 !~ /^#/' \
         crates/turbo-route-model/Cargo.toml | grep -v '^serde' || true)
if [[ -n "$deps" ]]; then
  bad "turbo-route-model must have zero dependencies (design §12.6)" \
      "found: $(echo "$deps" | tr '\n' ' ')" \
      "The model defines what the engine reasons about. Anything it can" \
      "name, the engine can reach — that is how source-awareness leaks back in."
else
  ok "turbo-route-model has no dependencies"
fi

# ---------------------------------------------------------------------
# 2. The engine does not depend on an adapter at build time.
#
# Adapters are *constructed* at the composition root (L5) and handed to
# the engine as `Arc<dyn Trait>`. A build-time edge from engine to
# adapter would mean the engine can construct its own inputs, which is
# exactly the composition-root-inside-the-engine flaw this design pass
# was corrected for. `[dev-dependencies]` is fine: tests may drive the
# engine through the real adapter.
# ---------------------------------------------------------------------
engine_deps=$(awk '/^\[dependencies\]/{f=1;next} /^\[/{f=0} f' \
                crates/turbo-tiles-pathfind/Cargo.toml)
if grep -q 'turbo-geodata' <<<"$engine_deps"; then
  bad "turbo-tiles-pathfind must not depend on an adapter crate (design §12.5)" \
      "Adapters are constructed at L5 and injected. A build-time edge here" \
      "lets the engine construct its own inputs."
else
  ok "engine has no build-time dependency on an adapter"
fi

# ---------------------------------------------------------------------
# 3. The concrete artifact type does not appear in the engine.
#
# The structural guard: if `Dem` is unnameable inside the engine, the
# concrete path is gone by construction — a type-level proof rather than
# an assertion. C4 removed the last exception (the projection shim), so
# this rule now admits none.
# ---------------------------------------------------------------------
hits=$(grep -rn 'turbo_tiles_elev' crates/turbo-tiles-pathfind/src/ || true)
if [[ -n "$hits" ]]; then
  bad "the engine names the concrete artifact crate (design §12, C1)" \
      "$(echo "$hits" | head -5)" \
      "Route it through a turbo-route-model port instead."
else
  ok "engine does not name the artifact crate at all"
fi

# ---------------------------------------------------------------------
# 3b. The engine names no coordinate reference system.
#
# C4. Rev. 1 of the design proposed `Projection` as an engine port —
# a port with exactly one implementation, which keeps the *concept* of
# a CRS inside the engine so every future feature gets to ask "which
# frame is this in?" until one of them answers wrongly. Removing the
# concept is stronger than abstracting it: the engine takes metres and
# returns metres. Comments count, because a comment asserting a frame
# is a claim the next reader will code against.
# ---------------------------------------------------------------------
crs=$(grep -rniE 'utm|wgs ?84|epsg|25833|4326|lon_deg|lat_deg' \
        crates/turbo-tiles-pathfind/src/ || true)
if [[ -n "$crs" ]]; then
  bad "the engine names a coordinate reference system (design §7.3, C4)" \
      "$(echo "$crs" | head -5)" \
      "The engine is planar-only. Projection is turbo-geo-frame, at L5."
else
  ok "engine names no coordinate reference system"
fi

# ---------------------------------------------------------------------
# 3c. Projection lives in exactly one crate.
#
# A projection scattered across handlers is one that eventually gets
# applied twice, or not at all, on some path nobody tested.
# ---------------------------------------------------------------------
if grep -rq 'fn wgs84_to_utm33n\|fn utm33n_to_wgs84' \
     --include='*.rs' crates/ --exclude-dir=turbo-geo-frame; then
  bad "the projection is defined outside turbo-geo-frame (design §7.3)" \
      "$(grep -rn 'fn wgs84_to_utm33n\|fn utm33n_to_wgs84' --include='*.rs' \
           crates/ --exclude-dir=turbo-geo-frame | head -3)"
else
  ok "projection defined only in turbo-geo-frame"
fi

# ---------------------------------------------------------------------
# 4. The adapter does not depend on the engine.
#
# Dependencies point inward: adapter -> model, never adapter -> engine.
# An adapter that knows the engine is a plugin, and plugins invert the
# ownership the composition root is supposed to have.
# ---------------------------------------------------------------------
if grep -q 'turbo-tiles-pathfind\|turbo-route-engine' \
     crates/turbo-geodata-artifacts/Cargo.toml; then
  bad "turbo-geodata-artifacts must not depend on the engine (design §12.5)" \
      "Adapters depend on turbo-route-model only."
else
  ok "adapter depends inward only"
fi

# ---------------------------------------------------------------------
# 5. The engine holds no concrete field type.
#
# `Arc<Dem>` in a struct field is the specific shape of the coupling C1
# removed. Catching the field declaration catches a regression before it
# spreads to the twelve holders it reached last time.
# ---------------------------------------------------------------------
holders=$(grep -rn 'Arc<Dem>\|Arc<turbo_tiles_elev::Dem>' \
            crates/turbo-tiles-pathfind/src/ || true)
if [[ -n "$holders" ]]; then
  bad "the engine holds a concrete DEM (design §12, C1)" "$(echo "$holders" | head -5)"
else
  ok "no concrete DEM holders in the engine"
fi

exit $fail
