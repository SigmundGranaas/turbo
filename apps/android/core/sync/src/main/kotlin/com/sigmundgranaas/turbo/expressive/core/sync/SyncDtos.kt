package com.sigmundgranaas.turbo.expressive.core.sync

import kotlinx.serialization.Serializable
import java.time.Instant
import java.time.OffsetDateTime

/** ISO-8601 ⇄ epoch-ms. The backend speaks ISO-8601; Room rows store epoch ms. */
internal object Iso8601 {
    fun toEpochMs(s: String?): Long? {
        if (s.isNullOrBlank()) return null
        return runCatching { Instant.parse(s).toEpochMilli() }
            .recoverCatching { OffsetDateTime.parse(s).toInstant().toEpochMilli() }
            .getOrNull()
    }

    fun fromEpochMs(ms: Long): String = Instant.ofEpochMilli(ms).toString()
}

/** A server delete: appears in the `deleted` array of every delta response. */
@Serializable
data class TombstoneDto(
    val id: String,
    val deletedAt: String? = null,
    val version: Long = 0,
)

// ─────────────────────────── Tracks ───────────────────────────

@Serializable
data class WirePoint(val longitude: Double, val latitude: Double)

@Serializable
data class TrackGeometryDto(
    val points: List<WirePoint> = emptyList(),
    val elevations: List<Double>? = null,
)

@Serializable
data class TrackMetadataDto(
    val name: String? = null,
    val description: String? = null,
    val colorHex: String? = null,
    val iconKey: String? = null,
    val lineStyleKey: String? = null,
    val smoothing: Boolean? = null,
)

@Serializable
data class TrackStatsDto(
    val distanceMeters: Double? = null,
    val ascentMeters: Double? = null,
    val descentMeters: Double? = null,
    val movingTimeSeconds: Int? = null,
    val recordedAt: String? = null,
)

@Serializable
data class TrackResponseDto(
    val id: String,
    val geometry: TrackGeometryDto = TrackGeometryDto(),
    val metadata: TrackMetadataDto = TrackMetadataDto(),
    val stats: TrackStatsDto? = null,
    val createdAt: String? = null,
    val updatedAt: String? = null,
    val version: Long = 0,
)

@Serializable
data class TracksDeltaDto(
    val items: List<TrackResponseDto> = emptyList(),
    val deleted: List<TombstoneDto> = emptyList(),
    val serverTime: String? = null,
    val nextCursor: String? = null,
)

/** POST/PUT body — PUT treats every field as optional (a partial update). */
@Serializable
data class TrackWriteRequest(
    val geometry: TrackGeometryDto,
    val metadata: TrackMetadataDto,
    val stats: TrackStatsDto? = null,
)

/** 412 body: the server's authoritative copy to adopt. */
@Serializable
data class TrackConflictDto(
    val currentVersion: Long? = null,
    val current: TrackResponseDto? = null,
)
