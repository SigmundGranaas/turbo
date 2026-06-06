package com.sigmundgranaas.turbo.expressive.domain

/**
 * A place the user has previously picked from search, surfaced as a quick
 * re-entry list when the search field is empty. Most-recent-first, de-duplicated
 * by name + rounded position, capped to a small window.
 */
data class RecentSearch(
    val name: String,
    val sub: String,
    val lat: Double,
    val lng: Double,
)
