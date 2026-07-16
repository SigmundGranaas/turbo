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
    /** Display colour as "#RRGGBB", null = the default track colour. Synced
     *  (`TrackMetadataDto.colorHex`) so a colour picked on any client renders
     *  everywhere. */
    val colorHex: String? = null,
    /** Icon key chosen for the track (web edits this today); pass-through synced. */
    val iconKey: String? = null,
    /** Line-style key (`solid`/`dotted`/`dashed`/`dash_dot`); pass-through synced.
     *  Android renders tracks as 3D tubes, which draw solid regardless — the key is
     *  kept so a style picked on another client survives an Android edit. */
    val lineStyleKey: String? = null,
)
