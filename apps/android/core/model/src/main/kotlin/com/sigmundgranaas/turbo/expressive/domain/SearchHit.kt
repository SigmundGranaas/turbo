package com.sigmundgranaas.turbo.expressive.domain

/** A place-search result (e.g. from Kartverket stedsnavn). */
data class SearchHit(
    val name: String,
    val description: String,
    val position: LatLng,
)
