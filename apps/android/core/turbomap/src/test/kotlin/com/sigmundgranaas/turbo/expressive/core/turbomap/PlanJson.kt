package com.sigmundgranaas.turbo.expressive.core.turbomap

/**
 * One `start` entry parsed from a minted streaming-plan JSON (plan P5.1):
 * `{"start":[{"id","kind","layer","z","x","y"}],"cancel":[ids]}`. The engine
 * plans; a test host drains the plan and delivers via `ingest*`. Minting is
 * consume-once — a start moves to `Fetching` in the engine's lifecycle table,
 * so it appears in exactly one plan until it's delivered, failed, or cancelled.
 */
internal data class PlanStart(
    val id: Long,
    val kind: String,
    val layer: String,
    val z: UByte,
    val x: UInt,
    val y: UInt,
)

private val START_RE =
    Regex("""\{"id":(\d+),"kind":"([^"]+)","layer":"([^"]+)","z":(\d+),"x":(\d+),"y":(\d+)}""")

/** All `start` entries in a plan JSON document (in priority order). */
internal fun planStarts(planJson: String): List<PlanStart> =
    START_RE.findAll(planJson).map { m ->
        PlanStart(
            id = m.groupValues[1].toLong(),
            kind = m.groupValues[2],
            layer = m.groupValues[3],
            z = m.groupValues[4].toUByte(),
            x = m.groupValues[5].toUInt(),
            y = m.groupValues[6].toUInt(),
        )
    }.toList()
