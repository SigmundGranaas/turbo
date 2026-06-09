package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.OverlayId
import java.nio.charset.StandardCharsets
import java.util.Base64

/**
 * MapLibre persists an opaque `byte[]` per offline region. Historically we stored
 * just the display name; this encodes the *full* region metadata (name, base map,
 * overlays, extent, zoom span, creation time) so it round-trips and the offline
 * list can show where a region is and what it covers.
 *
 * Pure (no MapLibre / Android dependency) so it is unit-testable on the JVM. The
 * format is a versioned, `;`-delimited `key=value` line; the free-text name is
 * Base64-encoded so place names with delimiters or unicode survive intact. A blob
 * that doesn't start with `v=` is treated as a legacy bare-name string.
 */
internal object OfflineRegionMetadata {

    data class Meta(
        val name: String,
        val base: BaseLayer,
        val overlays: Set<OverlayId>,
        val bounds: GeoBounds?,
        val minZoom: Double,
        val maxZoom: Double,
        val createdAtEpochMs: Long,
    )

    private const val VERSION = 2

    fun encode(meta: Meta): ByteArray {
        val name64 = Base64.getEncoder().encodeToString(meta.name.toByteArray(StandardCharsets.UTF_8))
        val ov = meta.overlays.joinToString(",") { it.name }
        val b = meta.bounds
        val bounds = if (b == null) "" else "${b.south},${b.west},${b.north},${b.east}"
        val line = buildString {
            append("v=").append(VERSION)
            append(";name=").append(name64)
            append(";base=").append(meta.base.id)
            append(";ov=").append(ov)
            append(";b=").append(bounds)
            append(";z=").append(meta.minZoom).append(',').append(meta.maxZoom)
            append(";t=").append(meta.createdAtEpochMs)
        }
        return line.toByteArray(StandardCharsets.UTF_8)
    }

    /** Decodes [bytes]; never throws — returns null only when nothing usable is present. */
    fun decode(bytes: ByteArray?): Meta? {
        if (bytes == null || bytes.isEmpty()) return null
        val text = String(bytes, StandardCharsets.UTF_8)
        if (!text.startsWith("v=")) {
            // Legacy region: the whole blob was the display name.
            return Meta(text, BaseLayer.Norgeskart, emptySet(), null, 0.0, 0.0, 0L)
        }
        val fields = text.split(';')
            .mapNotNull { part -> part.split('=', limit = 2).takeIf { it.size == 2 }?.let { it[0] to it[1] } }
            .toMap()
        val name = fields["name"]?.let {
            runCatching { String(Base64.getDecoder().decode(it), StandardCharsets.UTF_8) }.getOrNull()
        } ?: return null
        val base = BaseLayer.entries.firstOrNull { it.id == fields["base"] } ?: BaseLayer.Norgeskart
        val overlays = fields["ov"].orEmpty().split(',')
            .filter { it.isNotBlank() }
            .mapNotNull { id -> OverlayId.entries.firstOrNull { it.name == id } }
            .toSet()
        val bounds = fields["b"].orEmpty().split(',').mapNotNull { it.toDoubleOrNull() }
            .takeIf { it.size == 4 }
            ?.let { GeoBounds(south = it[0], west = it[1], north = it[2], east = it[3]) }
        val zoom = fields["z"].orEmpty().split(',').mapNotNull { it.toDoubleOrNull() }
        val minZoom = zoom.getOrNull(0) ?: 0.0
        val maxZoom = zoom.getOrNull(1) ?: 0.0
        val created = fields["t"]?.toLongOrNull() ?: 0L
        return Meta(name, base, overlays, bounds, minZoom, maxZoom, created)
    }
}
