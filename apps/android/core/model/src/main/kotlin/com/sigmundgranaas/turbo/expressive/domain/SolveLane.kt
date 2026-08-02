package com.sigmundgranaas.turbo.expressive.domain

/**
 * *Why* a given engine answered — not just which one did.
 *
 * [RouteSolveRecord.engine] alone cannot answer the question release 2
 * rests on. "Device" covers three completely different situations: a
 * tester forcing it, a walker with no signal, and the server timing out
 * on a connection that claimed to work. Only the third is the fallback
 * rate. Counting all three together would make the feature look like it
 * fires constantly in aeroplane mode and never on a bad tower, which is
 * the opposite of what the number is for.
 *
 * The same goes for the server side: [NoPack] and [ServerAnswered] are
 * both "server", but the first one is a **coverage miss** — someone
 * routed outside every pack they have downloaded. The plan calls that
 * out as a UX bug rather than an engine one, and it is invisible unless
 * the lane is recorded at the moment the decision is made.
 */
enum class SolveLane {
    /** A [RouteEngine] override. Excluded from every rate — a tester's
     *  deliberate choice is not evidence about automatic behaviour. */
    Forced,

    /** No network, and a pack covered the request. The easy case. */
    Offline,

    /**
     * No pack covers the request, so the server ran unconditionally.
     *
     * The coverage-miss signal. A high rate means the download flow is
     * not steering people to the regions they actually route in.
     */
    NoPack,

    /** The server answered inside its window. The healthy path. */
    ServerAnswered,

    /**
     * The server ran out of time and the phone answered instead.
     *
     * The fallback-rate numerator, and the single number that says
     * whether release 1 bought anything.
     */
    ServerTimedOut,

    /** The server failed outright (transport, not "no route") and the
     *  phone answered. Counted with [ServerTimedOut] as a fallback. */
    ServerErrored;

    /** Did the phone rescue a request the server was supposed to serve? */
    val isFallback: Boolean
        get() = this == ServerTimedOut || this == ServerErrored

    /** Was this an automatic decision, i.e. evidence about shipped behaviour? */
    val isAutomatic: Boolean
        get() = this != Forced
}

/**
 * How far apart the two engines' answers were, when both ran.
 *
 * The requirement is **equivalence, not bit-identity**. The two routers
 * read the same pack through the same solver, but the server's graph is
 * cut nationally and the device's per region, so a boundary edge can
 * differ legitimately. A hash would flag every one of those; what
 * matters is whether a walker would notice.
 *
 * So: length delta, and discrete Fréchet distance — the standard "how
 * far apart are these two curves" measure, which unlike a per-point
 * comparison is defined for polylines with different point counts, and
 * unlike Hausdorff respects the order you walk them in.
 */
data class RouteDivergence(
    val serverLengthM: Double,
    val deviceLengthM: Double,
    /** Worst-case separation between the two lines, in metres. */
    val frechetM: Double,
) {
    val lengthDeltaM: Double get() = kotlin.math.abs(serverLengthM - deviceLengthM)

    /**
     * Would a walker notice?
     *
     * 50 m is a deliberate, arguable threshold: about the point at which
     * two lines are visibly different trails on a phone screen at
     * walking zoom, rather than the same trail drawn slightly
     * differently. It is a reporting threshold, not a correctness one —
     * nothing fails because of it.
     */
    val isSignificant: Boolean get() = frechetM > SIGNIFICANT_M

    companion object {
        const val SIGNIFICANT_M = 50.0
    }
}
