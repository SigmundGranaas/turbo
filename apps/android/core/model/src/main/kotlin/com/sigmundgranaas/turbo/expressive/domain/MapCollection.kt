package com.sigmundgranaas.turbo.expressive.domain

/**
 * A user-defined grouping of map entities (markers, tracks). Local-only — a
 * lightweight folder with a colour + optional icon and a membership count.
 */
data class MapCollection(
    val id: String,
    val name: String,
    val colorArgb: Long? = null,
    val icon: String? = null,
    val itemCount: Int = 0,
)

/** The kinds of entity a collection can contain. */
enum class CollectionItemType { Marker, Path }
