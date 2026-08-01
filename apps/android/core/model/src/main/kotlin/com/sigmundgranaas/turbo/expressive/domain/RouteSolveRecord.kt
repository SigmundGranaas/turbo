package com.sigmundgranaas.turbo.expressive.domain

/**
 * What one route request cost and who answered it.
 *
 * This is the M1 deliverable in data form. The question "is on-device
 * routing fast enough to lead with" has never been answered against
 * real silicon — every figure quoted for the device path is
 * extrapolated from a desktop — and it cannot be answered by a tester
 * watching a line appear. It needs a number, per solve, attributable to
 * an engine.
 *
 * Kept deliberately small. This is not [the telemetry U3 describes][1]:
 * there is no upload, no session, no sampling, and nothing leaves the
 * phone. It is a readout, held in memory, showing the last few solves —
 * enough to write down after a walk, and not enough to be a privacy
 * question. U3 replaces it with something aggregated; until then this is
 * what stops release 2 resting on a hunch.
 *
 * [1]: apps/android/docs/on-device-routing-ux-plan.md
 */
data class RouteSolveRecord(
    /** Which router actually produced the answer. Never [RouteEngine.Auto]. */
    val engine: RouteEngine,
    /** Wall time from request to terminal event, in milliseconds. */
    val durationMs: Long,
    /** How many waypoints were asked for, including origin and destination. */
    val waypoints: Int,
    /**
     * Straight-line distance across the request, in kilometres.
     *
     * The distance *bucket* is what device p95 has to be reported
     * against — a 2 km solve and a 40 km solve are different questions —
     * and the requested span is the only distance known before the
     * solver answers, so it is the one that can be attached to a
     * failure as well as a success.
     */
    val spanKm: Double,
    val outcome: Outcome,
    /** Failure text, when [outcome] is [Outcome.Failed]. */
    val detail: String? = null,
) {
    enum class Outcome {
        /** A route came back. */
        Ok,

        /** The engine answered, and the answer was "no route". Not a defect. */
        NoRoute,

        /**
         * The engine could not be asked, or threw.
         *
         * The one that matters most on a release APK: a stripped native
         * library, a missing `.so` for this ABI, or JNA failing to
         * reflect over minified bindings all land here, and all of them
         * are invisible in a debug build.
         */
        Failed,
    }
}
