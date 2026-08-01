package com.sigmundgranaas.turbo.expressive.domain

/**
 * The four numbers [the plan][1] says release 2's decision rests on,
 * derived from a list of [RouteSolveRecord].
 *
 * A pure function of the records rather than a set of running counters.
 * Counters drift: they are updated at the call site, so a new lane or an
 * early return silently stops incrementing one of them, and nothing
 * ever says so. Deriving means a record that exists is a record that
 * counts, and the only way to under-count is to fail to record at all —
 * which is one place to check instead of six.
 *
 * [1]: apps/android/docs/on-device-routing-ux-plan.md
 */
data class RouteSolveStats(
    val total: Int,
    /** p95 device latency per distance bucket — the number that decides
     *  whether release 2 is safe. Buckets with no samples are absent. */
    val devicePercentiles: Map<DistanceBucket, LatencySummary>,
    val serverPercentiles: Map<DistanceBucket, LatencySummary>,
    /**
     * Of the automatic solves that COULD have fallen back, how many did.
     *
     * The denominator is the subtle part: it excludes forced solves
     * (not automatic), offline solves (no server was tried), and
     * no-pack solves (no fallback was possible). Including any of them
     * would move the rate for reasons that have nothing to do with the
     * server's reliability, which is what this measures.
     */
    val fallbackRate: Double,
    val fallbackEligible: Int,
    /** Automatic solves where no pack covered the request. A UX signal. */
    val coverageMissRate: Double,
    val failureRate: Double,
    /** Divergences seen, when shadow comparison was on. */
    val divergences: List<RouteDivergence>,
) {
    val worstDivergenceM: Double get() = divergences.maxOfOrNull { it.frechetM } ?: 0.0
    val significantDivergences: Int get() = divergences.count { it.isSignificant }

    companion object {
        fun from(records: List<RouteSolveRecord>): RouteSolveStats {
            val automatic = records.filter { it.lane.isAutomatic }
            // Only solves where a fallback was actually on the table.
            val eligible = automatic.filter {
                it.lane == SolveLane.ServerAnswered || it.lane.isFallback
            }
            return RouteSolveStats(
                total = records.size,
                devicePercentiles = summarise(records.filter { it.engine == RouteEngine.Device }),
                serverPercentiles = summarise(records.filter { it.engine == RouteEngine.Server }),
                fallbackRate = rate(eligible.count { it.lane.isFallback }, eligible.size),
                fallbackEligible = eligible.size,
                coverageMissRate = rate(
                    automatic.count { it.lane == SolveLane.NoPack },
                    automatic.size,
                ),
                failureRate = rate(
                    records.count { it.outcome == RouteSolveRecord.Outcome.Failed },
                    records.size,
                ),
                divergences = records.mapNotNull { it.divergence },
            )
        }

        private fun rate(n: Int, d: Int): Double = if (d == 0) 0.0 else n.toDouble() / d

        private fun summarise(rs: List<RouteSolveRecord>): Map<DistanceBucket, LatencySummary> =
            rs.groupBy { DistanceBucket.of(it.spanKm) }
                .mapValues { (_, group) -> LatencySummary.of(group.map { it.durationMs }) }
    }
}

/**
 * Distance buckets for latency reporting.
 *
 * A 2 km solve and a 40 km solve are different questions, and a single
 * p95 across both is dominated by whichever the tester happened to do
 * more of — which is why the plan asks for p95 *by bucket* rather than
 * a headline number.
 */
enum class DistanceBucket(val label: String) {
    Under2("<2 km"),
    Under10("2-10 km"),
    Under25("10-25 km"),
    Over25(">25 km");

    companion object {
        fun of(spanKm: Double): DistanceBucket = when {
            spanKm < 2.0 -> Under2
            spanKm < 10.0 -> Under10
            spanKm < 25.0 -> Under25
            else -> Over25
        }
    }
}

/** Latency for one bucket. [p95] is the headline; [n] is whether to believe it. */
data class LatencySummary(val n: Int, val medianMs: Long, val p95Ms: Long, val maxMs: Long) {
    companion object {
        fun of(durations: List<Long>): LatencySummary {
            val s = durations.sorted()
            return LatencySummary(
                n = s.size,
                medianMs = s.percentile(0.50),
                p95Ms = s.percentile(0.95),
                maxMs = s.lastOrNull() ?: 0L,
            )
        }

        /**
         * Nearest-rank percentile.
         *
         * Chosen over interpolation because the sample is tiny — a p95
         * over 20 records is the top one or two — and interpolating
         * between two real measurements to invent a number that was
         * never observed is a worse lie at this size than picking the
         * observation that actually happened.
         */
        private fun List<Long>.percentile(q: Double): Long {
            if (isEmpty()) return 0L
            val rank = kotlin.math.ceil(q * size).toInt().coerceIn(1, size)
            return this[rank - 1]
        }
    }
}
