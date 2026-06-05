package com.sigmundgranaas.turbo.expressive.domain

/**
 * In-memory sample content seeded from the design (Tromsø / Lyngen region).
 * Used to seed the Room database on first run so the map is never empty.
 */
object SampleData {

    /** Where the map first centres — Tromsø area, Troms. */
    val initialCamera = LatLng(69.6480, 18.9560)
    const val initialZoom = 10.5

    val markers = listOf(
        Marker("m-sjurfjellet", "Sjurfjellet", ActivityKindId.Cabin, LatLng(69.6412, 20.1003)),
        Marker("m-storsteinen", "Storsteinen", ActivityKindId.Mountain, LatLng(69.6695, 18.9890)),
        Marker("m-stor-bjornen", "Stor-Bjørnen", ActivityKindId.Viewpoint, LatLng(69.4220, 17.9650)),
        Marker("m-fjellheisen", "Fjellheisen Kafé", ActivityKindId.Cafe, LatLng(69.6360, 18.9990)),
        Marker("m-tromsdalstind", "Tromsdalstind", ActivityKindId.Hiking, LatLng(69.6190, 19.0900)),
    )

    /** A recorded track (Storsteinen Loop) for the path / recording screens. */
    val storsteinenLoop = listOf(
        LatLng(69.6480, 18.9560),
        LatLng(69.6560, 18.9700),
        LatLng(69.6620, 18.9820),
        LatLng(69.6695, 18.9890),
        LatLng(69.6660, 19.0010),
        LatLng(69.6585, 19.0040),
        LatLng(69.6512, 18.9905),
        LatLng(69.6480, 18.9560),
    )

    data class ConditionsTile(val label: String, val value: String)

    /** "Conditions now" mini-card values for the marker-info sheet. */
    val conditionsNow = listOf(
        ConditionsTile("Temp", "-3°"),
        ConditionsTile("Wind NW", "4 m/s"),
        ConditionsTile("mm/h", "0.2"),
    )
}
