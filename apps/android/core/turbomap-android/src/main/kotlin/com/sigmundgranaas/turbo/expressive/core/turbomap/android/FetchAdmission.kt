package com.sigmundgranaas.turbo.expressive.core.turbomap.android

/**
 * Host-side ADMISSION for one streaming-plan `start` (plan P5.1).
 *
 * The engine plans (priority order, in-flight dedup via its lifecycle table,
 * cancels); the host only decides whether it can run the fetch RIGHT NOW.
 * The two policies that deliberately live above the engine's table:
 *  - per-kind fetch lanes ([laneCap] vs [laneUsed]) — the narrow MVT lane
 *    that keeps the tileserver inside its timeout, DEM separate from raster;
 *  - failure backoff ([retryAt] > [now]) — the engine re-pends a failed
 *    tile immediately by design ("retry policy lives above the table").
 *
 * A declined start MUST be reported via `nativeReportFetchCancelled`, so the
 * engine re-issues it on a later plan — declining never loses a tile.
 */
internal fun admitFetch(
    key: String,
    laneUsed: Int,
    laneCap: Int,
    alreadyInFlight: Boolean,
    retryAt: Map<String, Long>,
    now: Long,
): Boolean {
    if (alreadyInFlight) return false
    if (laneUsed >= laneCap) return false
    if ((retryAt[key] ?: 0L) > now) return false
    return true
}
