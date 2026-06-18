package com.sigmundgranaas.turbo.expressive.domain

import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.PhaseSplit

/** A named, persisted [GeoPath] — a recorded track or saved route, optionally
 *  tagged with the [activityKind] it represents (hiking, skiing, fishing…).
 *
 *  When the track came from following a planned route (D1), [plannedRoute] keeps
 *  the guide geometry it was walked against and [phaseSplits] the checkpoint splits
 *  recorded along the way, so the saved artifact can redraw the guide + splits later.
 *  Both are empty/null for plain recordings. */
data class SavedPath(
    val id: String,
    val name: String,
    val path: GeoPath,
    val activityKind: ActivityKindId? = null,
    val plannedRoute: List<LatLng>? = null,
    val phaseSplits: List<PhaseSplit> = emptyList(),
)
