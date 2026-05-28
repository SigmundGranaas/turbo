#!/usr/bin/env bash
# Primitives E2E. Spins up a clean database, seeds synthetic data,
# builds every artifact via the CLI, boots the server, and curls every
# primitive endpoint to verify the response is what the user would see.
#
# Exit 0 means: every primitive returned a sensible answer against
# real artifacts. Exit non-zero means we have a regression — the
# message will point at the failed assertion.
#
# Usage:
#   tools/e2e/run.sh            # full run, ~60 s
#   tools/e2e/run.sh --keep     # leave artifacts + DB around for poking

set -euo pipefail

# psql lives under libpq on macOS, not on the default PATH.
export PATH="/opt/homebrew/opt/libpq/bin:$PATH"

KEEP=0
for arg in "$@"; do
    case "$arg" in
        --keep) KEEP=1 ;;
        *) echo "unknown arg: $arg" >&2; exit 2 ;;
    esac
done

# The persistent test container created by run-e2e.sh hosts every
# primitive E2E run. We don't touch its existing schema — we use
# our own database `tiles_e2e` so we can DROP / CREATE without
# affecting concurrent Rust integration tests.
ADMIN_DB="postgres://postgres:testpass@localhost:55433/postgres"
DB="postgres://postgres:testpass@localhost:55433/tiles_e2e"
ARTIFACTS="/tmp/turbo-e2e-artifacts"
TILESERVER_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
PORT=8090
BASE="http://127.0.0.1:${PORT}"
LOG="/tmp/turbo-e2e-server.log"

cd "${TILESERVER_DIR}"

# The auth layer is wired into every router and refuses to start
# without a JWT secret, even though the primitive endpoints don't
# require auth. Stub one in for the test process.
export JWT_SECRET="e2e-not-a-real-secret-do-not-use-anywhere"

# Track each assertion as it lands. The trap at the bottom prints a
# summary line that fails the run if any assertion is missing.
assertions=()
failures=()
note() { echo -e "\033[1;34m▶\033[0m $*"; }
ok()   { echo -e "  \033[32m✓\033[0m $*"; assertions+=("$*"); }
fail() { echo -e "  \033[31m✗\033[0m $*"; failures+=("$*"); }

server_pid=""
cleanup() {
    local exit_code=$?
    if [[ -n "${server_pid}" ]]; then
        kill "${server_pid}" 2>/dev/null || true
        wait "${server_pid}" 2>/dev/null || true
    fi
    if [[ "${KEEP}" -eq 0 ]]; then
        psql "${ADMIN_DB}" -c "DROP DATABASE IF EXISTS tiles_e2e WITH (FORCE)" >/dev/null 2>&1 || true
        rm -rf "${ARTIFACTS}"
    fi
    # If the script aborted before reaching the end (set -e), the
    # trap fires with a non-zero exit code and no recorded failures.
    # Synthesize one so the summary line tells the truth.
    if [[ "${exit_code}" -ne 0 && "${#failures[@]}" -eq 0 ]]; then
        failures+=("script aborted with exit code ${exit_code} (see output above)")
    fi
    echo ""
    echo "================================================================"
    if [[ "${#failures[@]}" -eq 0 && "${#assertions[@]}" -gt 0 ]]; then
        echo -e "  \033[32m✓ E2E SUITE PASS\033[0m — ${#assertions[@]} assertions"
        echo "================================================================"
        exit 0
    else
        echo -e "  \033[31m✗ E2E SUITE FAIL\033[0m — ${#assertions[@]} pass, ${#failures[@]} fail"
        printf '    - %s\n' "${failures[@]}"
        echo "  Server log: ${LOG}"
        echo "================================================================"
        exit 1
    fi
}
trap cleanup EXIT

# ---- 1. Prepare DB ------------------------------------------------------
note "Preparing tiles_e2e database"
psql "${ADMIN_DB}" -c "DROP DATABASE IF EXISTS tiles_e2e WITH (FORCE)" >/dev/null
psql "${ADMIN_DB}" -c "CREATE DATABASE tiles_e2e" >/dev/null
psql "${DB}" -c "
    CREATE EXTENSION IF NOT EXISTS postgis;
    CREATE EXTENSION IF NOT EXISTS postgis_raster;
    CREATE EXTENSION IF NOT EXISTS pgrouting;
    CREATE EXTENSION IF NOT EXISTS pg_trgm;
" >/dev/null
ok "extensions installed"

note "Applying migrations"
DATABASE_URL="${DB}" cargo run --quiet --bin tileserver -- migrate >/dev/null
ok "migrations applied"

# ---- 2. Seed -----------------------------------------------------------
note "Seeding synthetic data"
psql "${DB}" -v ON_ERROR_STOP=1 -f "${TILESERVER_DIR}/tools/e2e/seed.sql" >/dev/null
counts=$(psql "${DB}" -At -c "
    SELECT 'dem='||COUNT(*) FROM paths.dem
    UNION ALL SELECT 'nodes='||COUNT(*) FROM paths.node
    UNION ALL SELECT 'edges='||COUNT(*) FROM paths.edge
    UNION ALL SELECT 'water='||COUNT(*) FROM terrain.water_polygon
    UNION ALL SELECT 'anchors='||COUNT(*) FROM anchors.anchor
" | tr '\n' ' ')
ok "seed: ${counts}"

# ---- 3. Build artifacts ------------------------------------------------
note "Building artifacts under ${ARTIFACTS}"
rm -rf "${ARTIFACTS}"
mkdir -p "${ARTIFACTS}"

build_kind() {
    local kind="$1"
    # `cargo run` always emits its compile-status warnings on stderr
    # even with --quiet. Capture stdout (the report JSON) and stderr
    # separately so jq sees clean input.
    local stderr_file="/tmp/turbo-e2e-build-${kind}.err"
    local report
    if ! report=$(DATABASE_URL="${DB}" cargo run --quiet --bin tileserver -- \
        build-artifacts --kind="${kind}" --out="${ARTIFACTS}" 2>"${stderr_file}"); then
        fail "build ${kind}: $(tail -5 "${stderr_file}")"
        return 1
    fi
    local size
    size=$(echo "${report}" | jq -r '.file_size_bytes // "?"' 2>/dev/null || echo "?")
    ok "built ${kind} (${size} bytes)"
}

build_kind dem
build_kind graph
build_kind search
build_kind mask

note "Verifying artifacts"
DATABASE_URL="${DB}" cargo run --quiet --bin tileserver -- verify-artifacts --dir="${ARTIFACTS}" \
    > /tmp/turbo-e2e-verify.json 2>/tmp/turbo-e2e-verify.err
if jq -e '.dem.ok and .graph.ok and .anchors.ok and .mask.ok' /tmp/turbo-e2e-verify.json >/dev/null 2>&1; then
    ok "verify-artifacts: all primitives healthy ($(jq -c '{dem:.dem.centre_sample,graph:.graph.nodes,mask:.mask.cells_water,anchors:.anchors.anchors}' /tmp/turbo-e2e-verify.json))"
else
    fail "verify-artifacts failed (stdout: $(cat /tmp/turbo-e2e-verify.json | head -3); stderr: $(tail -3 /tmp/turbo-e2e-verify.err))"
fi

# ---- 4. Boot server ----------------------------------------------------
note "Booting tileserver on ${PORT} (no-db, artifacts-only)"
DATABASE_URL="${DB}" cargo run --quiet --bin tileserver -- serve \
    --bind="127.0.0.1:${PORT}" \
    --artifacts-dir="${ARTIFACTS}" \
    --public-base-url="${BASE}" \
    --no-db \
    > "${LOG}" 2>&1 &
server_pid=$!

for _ in $(seq 1 30); do
    if curl -sf "${BASE}/healthz" >/dev/null 2>&1; then
        ok "server up"
        break
    fi
    sleep 0.5
done
if ! curl -sf "${BASE}/healthz" >/dev/null; then
    fail "server failed to come up — see ${LOG}"
    tail -40 "${LOG}" >&2
    exit 1
fi

# Helper: POST JSON, fail if HTTP status is not 200.
http_post() {
    local path="$1" body="$2"
    local resp http
    resp=$(curl -sS -o /tmp/turbo-e2e-resp.json -w '%{http_code}' \
        -H 'content-type: application/json' \
        -X POST "${BASE}${path}" -d "${body}")
    http="${resp: -3}"
    if [[ "${http}" != "200" ]]; then
        echo "HTTP ${http}: $(cat /tmp/turbo-e2e-resp.json)" >&2
        return 1
    fi
    cat /tmp/turbo-e2e-resp.json
}
http_get() {
    local path="$1"
    local resp http
    resp=$(curl -sS -o /tmp/turbo-e2e-resp.json -w '%{http_code}' "${BASE}${path}")
    http="${resp: -3}"
    if [[ "${http}" != "200" ]]; then
        echo "HTTP ${http}: $(cat /tmp/turbo-e2e-resp.json)" >&2
        return 1
    fi
    cat /tmp/turbo-e2e-resp.json
}

# All lon/lat values below were computed once via PostGIS
# `ST_Transform(... , 4326)` from the EPSG:25833 metres in
# `tools/e2e/seed.sql`. Updating one means re-deriving both.

# Reference points used by the assertions:
#   tile A centre  (261_280, 6_651_280) → (10.7272, 59.9296)  elev 500
#   tile B centre  (263_840, 6_651_280) → (10.7729, 59.9311)  elev 550
#   A/B boundary   (262_560, 6_651_280) → (10.7501, 59.9303)  slope step
#   lake centre    (262_200, 6_652_200) → (10.7426, 59.9384)  water
#   dry point      (262_200, 6_651_400) → (10.7435, 59.9312)  none
#   node 1         (260_500, 6_652_000) → (10.7125, 59.9356)  graph
#   node 5         (262_500, 6_652_000) → (10.7482, 59.9368)  graph
#   cabin north    (261_500, 6_651_500) → (10.7309, 59.9317)  anchor
#   far east       (264_000, 6_651_900) → (10.7751, 59.9367)  off-graph
#   off-trail 1    (264_500, 6_651_900) → (10.7840, 59.9370)  off-graph
#   off-trail 2    (264_500, 6_651_400) → (10.7846, 59.9325)  off-graph

# ---- 5. Stage 1: Elevation --------------------------------------------
note "Stage 1: /v1/elev"

elev_a=$(http_post /v1/elev/sample '{"lon":10.7272,"lat":59.9296}' \
    | jq -r '.elev_m // 0') || true
if [[ -n "${elev_a}" && "${elev_a}" != "null" ]] && \
   awk "BEGIN { exit !(${elev_a} >= 499 && ${elev_a} <= 501) }"; then
    ok "elev tile A sample = ${elev_a} m (expect ≈500)"
else
    fail "elev tile A sample expected ~500 m, got ${elev_a}"
fi

elev_b=$(http_post /v1/elev/sample '{"lon":10.7729,"lat":59.9311}' \
    | jq -r '.elev_m // 0') || true
if [[ -n "${elev_b}" && "${elev_b}" != "null" ]] && \
   awk "BEGIN { exit !(${elev_b} >= 549 && ${elev_b} <= 551) }"; then
    ok "elev tile B sample = ${elev_b} m (expect ≈550)"
else
    fail "elev tile B sample expected ~550 m, got ${elev_b}"
fi

prof=$(http_post /v1/elev/profile \
    '{"line":[[10.7272,59.9296],[10.7729,59.9311]],"samples":20}')
prof_min=$(echo "${prof}" | jq '[.elev_m[]|select(. != null)]|min // 0')
prof_max=$(echo "${prof}" | jq '[.elev_m[]|select(. != null)]|max // 0')
if awk "BEGIN { exit !(${prof_min} >= 499 && ${prof_max} <= 551 && (${prof_max} - ${prof_min}) >= 40) }"; then
    ok "elev profile spans ${prof_min}–${prof_max} m (Δ ≥ 40)"
else
    fail "elev profile range ${prof_min}–${prof_max} unexpected"
fi

cov=$(http_get /v1/debug/elev/coverage)
cov_cells_x=$(echo "${cov}" | jq -r .cells_x)
cov_cells_y=$(echo "${cov}" | jq -r .cells_y)
if [[ "${cov_cells_x}" -ge 200 && "${cov_cells_y}" -ge 200 ]]; then
    ok "elev coverage: ${cov_cells_x} × ${cov_cells_y} cells"
else
    fail "elev coverage shape ${cov_cells_x}×${cov_cells_y}"
fi

# ---- 6. Stage 2: Slope ------------------------------------------------
note "Stage 2: /v1/slope"

slope_flat=$(http_post /v1/slope/sample '{"lon":10.7272,"lat":59.9296}' \
    | jq -r '.slope_deg // 999')
if [[ -n "${slope_flat}" && "${slope_flat}" != "null" ]] && \
   awk "BEGIN { exit !(${slope_flat} >= 0 && ${slope_flat} <= 1.0) }"; then
    ok "slope at flat tile centre = ${slope_flat}° (≈0)"
else
    fail "slope at flat centre expected ~0°, got ${slope_flat}"
fi

# Scan a few lons centred on the boundary and find the maximum
# observed slope. Robust to small UTM-conversion drift between
# the Rust pathfinder and PostGIS ST_Transform.
slope_max="0"
slope_max_lon=""
for lon in 10.74985 10.74999 10.75009 10.75019 10.75050 10.75100; do
    s=$(http_post /v1/slope/sample "{\"lon\":${lon},\"lat\":59.9303}" \
        | jq -r '.slope_deg // 0')
    if awk "BEGIN { exit !(${s} > ${slope_max}) }"; then
        slope_max=${s}
        slope_max_lon=${lon}
    fi
done
if awk "BEGIN { exit !(${slope_max} >= 20) }"; then
    ok "slope across A/B boundary peaks at ${slope_max}° (lon=${slope_max_lon}, ≥20)"
else
    fail "slope across A/B boundary peaked at only ${slope_max}°"
fi

# ---- 7. Stage 3: Mask -------------------------------------------------
note "Stage 3: /v1/mask"

mask_lake=$(http_post /v1/mask/sample '{"lon":10.7426,"lat":59.9384}' \
    | jq -r .kind)
if [[ "${mask_lake}" == "water" ]]; then
    ok "mask inside lake → water"
else
    fail "mask inside lake expected 'water', got '${mask_lake}'"
fi

mask_dry=$(http_post /v1/mask/sample '{"lon":10.7435,"lat":59.9312}' \
    | jq -r .kind)
if [[ "${mask_dry}" == "none" ]]; then
    ok "mask away from lake → none"
else
    fail "mask away from lake expected 'none', got '${mask_dry}'"
fi

mask_cov=$(http_get /v1/debug/mask/coverage)
water_cells=$(echo "${mask_cov}" | jq -r '.cells_water // 0')
if [[ "${water_cells}" -ge 1 ]]; then
    ok "mask coverage reports ${water_cells} water cells"
else
    fail "mask coverage reports zero water cells; expected ≥1"
fi

# ---- 8. Stage 4: Graph route ------------------------------------------
note "Stage 4: /v1/route"

# Snap test: pick lon/lat near node 1 (260_500, 6_652_000) and node 5
# (262_500, 6_652_000). Both should snap and route along the lattice.
route=$(http_post /v1/route '{
    "from":[10.7125,59.9356],
    "to":[10.7482,59.9368],
    "profile":"foot",
    "snap_radius_m":300
}')
length=$(echo "${route}" | jq -r '.length_m // 0')
geom_pts=$(echo "${route}" | jq -r '.geometry | length')
if awk "BEGIN { exit !(${length} >= 1500 && ${length} <= 3000) }" && [[ "${geom_pts}" -ge 3 ]]; then
    ok "graph route 1→5: length=${length} m, ${geom_pts} vertices"
else
    fail "graph route 1→5 wrong: length=${length}, geom=${geom_pts}"
fi

stats=$(http_get /v1/debug/graph/stats)
nodes=$(echo "${stats}" | jq -r '.meta.node_count')
edges=$(echo "${stats}" | jq -r '.meta.edge_count')
if [[ "${nodes}" -eq 25 && "${edges}" -ge 32 ]]; then
    ok "graph stats: ${nodes} nodes, ${edges} directed edges"
else
    fail "graph stats: got ${nodes} nodes, ${edges} edges; expected 25 + ≥32"
fi

# ---- 9. Stage 5: Search ----------------------------------------------
note "Stage 5: /v1/search"

# Nearest to cabin_north position (261_500, 6_651_500) → first
# anchor should be "Test Cabin North" itself.
near=$(http_post /v1/search/nearest '{"lon":10.7309,"lat":59.9317,"n":3}')
near_first=$(echo "${near}" | jq -r '.anchors[0].name')
if [[ "${near_first}" == "Test Cabin North" ]]; then
    ok "search nearest → ${near_first}"
else
    fail "search nearest expected 'Test Cabin North', got '${near_first}'"
fi

named=$(http_get '/v1/search/name?q=Test%20Cabin&limit=10')
named_count=$(echo "${named}" | jq -r '.anchors | length')
if [[ "${named_count}" -ge 2 ]]; then
    ok "search name 'Test Cabin' → ${named_count} hits"
else
    fail "search name 'Test Cabin' returned ${named_count} hits"
fi

# ---- 10. Stage 6: Pathfind --------------------------------------------
note "Stage 6: /v1/pathfind (all three strategies)"

layers_resp=$(http_get /v1/debug/pathfind/layers)
layer_count=$(echo "${layers_resp}" | jq -r '.layers | length')
if [[ "${layer_count}" -ge 3 ]]; then
    ok "pathfind layers registered: $(echo "${layers_resp}" | jq -c .layers)"
else
    fail "pathfind layers list: got ${layer_count} layers, expected ≥3"
fi

# Strategy 1: on_graph — both endpoints near graph nodes.
on_graph=$(http_post /v1/pathfind '{
    "from":[10.7125,59.9356],
    "to":[10.7482,59.9368],
    "prefs":{"profile":"foot","snap_radius_m":300}
}')
on_graph_strategy=$(echo "${on_graph}" | jq -r '.path.strategy')
on_graph_legs=$(echo "${on_graph}" | jq -r '.path.legs | length')
if [[ "${on_graph_strategy}" == "on_graph" && "${on_graph_legs}" -ge 1 ]]; then
    ok "pathfind on_graph: strategy=${on_graph_strategy}, legs=${on_graph_legs}"
else
    fail "pathfind on_graph wrong: strategy=${on_graph_strategy}"
fi

# Strategy 2: hybrid — `to` snaps to node 5, `from` sits at
# (264_000, 6_651_900) which is 1.5 km east of the nearest graph
# node (snap fails at 150 m, bridge catches it within 3 km).
hybrid=$(http_post /v1/pathfind '{
    "from":[10.7751,59.9367],
    "to":[10.7125,59.9356],
    "prefs":{"profile":"foot","snap_radius_m":150,"bridge_radius_m":3000}
}')
hybrid_strategy=$(echo "${hybrid}" | jq -r '.path.strategy')
hybrid_leg_kinds=$(echo "${hybrid}" | jq -r '[.path.legs[].kind] | join(",")')
if [[ "${hybrid_strategy}" == "hybrid" ]] && \
   echo "${hybrid_leg_kinds}" | grep -q "graph" && \
   echo "${hybrid_leg_kinds}" | grep -q "off_trail"; then
    ok "pathfind hybrid: strategy=${hybrid_strategy}, legs=[${hybrid_leg_kinds}]"
else
    fail "pathfind hybrid wrong: strategy=${hybrid_strategy} legs=[${hybrid_leg_kinds}]"
fi

# Strategy 3: off_trail — both endpoints far from any graph node
# AND tight bridge_radius so the hybrid path can't anchor. Two
# points inside the DEM extent east of the lattice.
off_trail=$(http_post /v1/pathfind '{
    "from":[10.7840,59.9370],
    "to":[10.7846,59.9325],
    "prefs":{"profile":"foot","snap_radius_m":50,"bridge_radius_m":80,"mesh_cell_m":50,"max_off_trail_km":5}
}')
off_trail_strategy=$(echo "${off_trail}" | jq -r '.path.strategy')
if [[ "${off_trail_strategy}" == "off_trail" || "${off_trail_strategy}" == "hybrid" ]]; then
    ok "pathfind off_trail/hybrid fallback: strategy=${off_trail_strategy}"
else
    fail "pathfind off_trail wrong: strategy=${off_trail_strategy}"
fi

# Layer override: refusing layer set to weight 0 means a path that
# would otherwise be vetoed by the lake polygon is now allowed.
# Endpoints are on opposite sides of the lake (262_100..262_300 m).
# from=(262_050, 6_652_200) ≈ (10.7417, 59.9384)
# to  =(262_350, 6_652_200) ≈ (10.7435, 59.9384)
no_mask=$(http_post /v1/pathfind '{
    "from":[10.7417,59.9384],
    "to":[10.7435,59.9384],
    "prefs":{
        "profile":"foot",
        "allow_off_trail":true,
        "snap_radius_m":50,
        "bridge_radius_m":50,
        "mesh_cell_m":50,
        "layer_weights":{"mask_refusal":0.0}
    }
}')
no_mask_strategy=$(echo "${no_mask}" | jq -r '.path.strategy')
if [[ "${no_mask_strategy}" == "off_trail" || "${no_mask_strategy}" == "hybrid" ]]; then
    ok "pathfind with mask_refusal=0 → ${no_mask_strategy} succeeds"
else
    fail "pathfind with mask_refusal=0 failed: ${no_mask_strategy}"
fi

note "all assertions completed"
