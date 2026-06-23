package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import java.io.File
import java.nio.charset.StandardCharsets
import java.util.Base64

/**
 * On-disk persistence for offline-region metadata, replacing MapLibre's opaque
 * per-region blob now that downloads land in the wgpu [TileStore] directly.
 *
 * One file per region (`<id>.region`) under [dir]. Line 1 reuses
 * [OfflineRegionMetadata] for the descriptive half (name / base / overlays /
 * extent / zoom span / created-at); line 2 carries the mutable download state
 * (status / progress / tile + byte counts / error). Pure JVM (File IO + String),
 * so it round-trips in unit tests without Android or MapLibre.
 */
internal class OfflineRegionStore(private val dir: File) {

    /** Every persisted region, newest first by creation time. */
    fun loadAll(): List<OfflineRegionInfo> {
        val files = dir.listFiles { f -> f.isFile && f.name.endsWith(SUFFIX) } ?: return emptyList()
        return files.mapNotNull { decode(it) }.sortedByDescending { it.createdAtEpochMs }
    }

    /** Persist (create or overwrite) [info]. */
    fun save(info: OfflineRegionInfo) {
        runCatching {
            dir.mkdirs()
            fileFor(info.id).writeText(encode(info), StandardCharsets.UTF_8)
        }
    }

    /** Forget a region's metadata (its tiles are pruned separately by the manager). */
    fun delete(id: Long) {
        runCatching { fileFor(id).delete() }
    }

    private fun fileFor(id: Long) = File(dir, "$id$SUFFIX")

    private fun encode(info: OfflineRegionInfo): String {
        val meta = OfflineRegionMetadata.encode(
            OfflineRegionMetadata.Meta(
                name = info.name,
                base = info.base,
                overlays = info.overlays,
                bounds = info.bounds,
                minZoom = info.minZoom,
                maxZoom = info.maxZoom,
                createdAtEpochMs = info.createdAtEpochMs,
            ),
        ).toString(StandardCharsets.UTF_8)
        val err = info.errorReason
            ?.let { Base64.getEncoder().encodeToString(it.toByteArray(StandardCharsets.UTF_8)) }
            .orEmpty()
        val state = buildString {
            append("id=").append(info.id)
            append(";st=").append(info.status.name)
            append(";p=").append(info.progress)
            append(";tiles=").append(info.tileCount)
            append(";bytes=").append(info.sizeBytes)
            append(";err=").append(err)
        }
        return "$meta\n$state"
    }

    private fun decode(file: File): OfflineRegionInfo? {
        val lines = runCatching { file.readText(StandardCharsets.UTF_8) }.getOrNull()?.split('\n') ?: return null
        if (lines.size < 2) return null
        val meta = OfflineRegionMetadata.decode(lines[0].toByteArray(StandardCharsets.UTF_8)) ?: return null
        val fields = lines[1].split(';')
            .mapNotNull { part -> part.split('=', limit = 2).takeIf { it.size == 2 }?.let { it[0] to it[1] } }
            .toMap()
        val id = fields["id"]?.toLongOrNull() ?: file.nameWithoutExtension.toLongOrNull() ?: return null
        val status = OfflineStatus.entries.firstOrNull { it.name == fields["st"] } ?: OfflineStatus.Failed
        val err = fields["err"]?.takeIf { it.isNotBlank() }
            ?.let { runCatching { String(Base64.getDecoder().decode(it), StandardCharsets.UTF_8) }.getOrNull() }
        return OfflineRegionInfo(
            id = id,
            name = meta.name,
            status = status,
            progress = fields["p"]?.toFloatOrNull() ?: 0f,
            sizeBytes = fields["bytes"]?.toLongOrNull() ?: 0L,
            tileCount = fields["tiles"]?.toLongOrNull() ?: 0L,
            base = meta.base,
            overlays = meta.overlays,
            bounds = meta.bounds,
            minZoom = meta.minZoom,
            maxZoom = meta.maxZoom,
            createdAtEpochMs = meta.createdAtEpochMs,
            errorReason = err,
        )
    }

    private companion object {
        const val SUFFIX = ".region"
    }
}
