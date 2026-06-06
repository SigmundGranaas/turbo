package com.sigmundgranaas.turbo.expressive.domain

/** How a place name relates to the queried point. */
enum class PlaceQualifier { On, In, Near, At }

/**
 * A human description of a coordinate, resolved by reverse-geocoding: a primary
 * [title] (the nearest meaningful place name), an optional [qualifier] ("On
 * Galdhøpiggen", "In Lom"), an optional [secondary] line (feature kind, kommune,
 * or address detail) and an optional [elevationM]. Mirrors the Flutter
 * `LocationDescription`.
 */
data class LocationDescription(
    val title: String,
    val qualifier: PlaceQualifier? = null,
    val secondary: String? = null,
    val elevationM: Double? = null,
) {
    /** Headline, e.g. "On Galdhøpiggen" / "In Lom". */
    val label: String
        get() = when (qualifier) {
            PlaceQualifier.On -> "On $title"
            PlaceQualifier.In -> "In $title"
            PlaceQualifier.Near -> "Near $title"
            PlaceQualifier.At, null -> title
        }

    /** Supporting line, e.g. "fjelltopp · 2469 m · Lom". */
    val subtitle: String
        get() = listOfNotNull(
            secondary?.takeIf(String::isNotBlank),
            elevationM?.let { "${it.toInt()} m" },
        ).joinToString(" · ")
}
