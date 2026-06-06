package com.sigmundgranaas.turbo.expressive.domain

import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath

/** A named, persisted [GeoPath] — a recorded track or saved route, optionally
 *  tagged with the [activityKind] it represents (hiking, skiing, fishing…). */
data class SavedPath(
    val id: String,
    val name: String,
    val path: GeoPath,
    val activityKind: ActivityKindId? = null,
)
