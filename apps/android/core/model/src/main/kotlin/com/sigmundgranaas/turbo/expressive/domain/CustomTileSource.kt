package com.sigmundgranaas.turbo.expressive.domain

/**
 * A user-supplied XYZ raster basemap ("add your own map URL"). Persisted in
 * settings; when [UserSettings.selectedCustomSourceId] points at one, it
 * replaces the built-in [BaseLayer] as the base raster. XYZ templates only —
 * the wgpu engine substitutes `{z}/{x}/{y}` (WMS needs engine work; out of scope).
 */
data class CustomTileSource(
    val id: String,
    val name: String,
    val urlTemplate: String,
    /** Deepest zoom the pyramid serves; the engine over-zooms past it. */
    val maxZoom: Int = DEFAULT_MAX_ZOOM,
) {
    companion object {
        const val DEFAULT_MAX_ZOOM = 19

        /** True when [url] is a usable XYZ template: http(s) scheme and all of
         *  the `{z}`/`{x}`/`{y}` placeholders present. */
        fun isValidTemplate(url: String): Boolean {
            val u = url.trim()
            val scheme = u.startsWith("http://") || u.startsWith("https://")
            return scheme && listOf("{z}", "{x}", "{y}").all { it in u }
        }
    }
}
