package com.sigmundgranaas.turbo.expressive.core.turbomap.android

/** One reconcile pass's decision: which tiles to start fetching, which in-flight fetches to cancel. */
internal data class ReconcileDecision(val toStart: List<String>, val toCancel: List<String>)

/**
 * Pure tile-reconcile policy — the heart of the host tile pipeline, kept free of
 * the native engine / coroutines / HTTP so it can be unit-tested directly.
 *
 * Given the engine's [desiredOrdered] tiles (nearest-first) and what's currently
 * [inFlight]:
 *  - cancel in-flight fetches no longer desired (the camera left them behind) —
 *    which frees their concurrency slots **in this same pass**, so a fast pan
 *    can't starve the current viewport behind stale work;
 *  - start the nearest desired-not-in-flight tiles up to [cap], skipping any
 *    still inside a post-failure backoff window ([retryAt] > [now]).
 *
 * Because a tile that failed simply isn't present, it reappears in
 * [desiredOrdered] on the next pass and is retried for free once its backoff
 * elapses — so loading is self-healing and never permanently stalls.
 */
internal fun planReconcile(
    desiredOrdered: List<String>,
    inFlight: Set<String>,
    retryAt: Map<String, Long>,
    now: Long,
    cap: Int,
): ReconcileDecision {
    val desired = desiredOrdered.toHashSet()
    val toCancel = inFlight.filter { it !in desired }
    val remaining = inFlight.size - toCancel.size
    val slots = (cap - remaining).coerceAtLeast(0)
    val toStart = desiredOrdered.asSequence()
        .filter { it !in inFlight && (retryAt[it] ?: 0L) <= now }
        .take(slots)
        .toList()
    return ReconcileDecision(toStart, toCancel)
}
