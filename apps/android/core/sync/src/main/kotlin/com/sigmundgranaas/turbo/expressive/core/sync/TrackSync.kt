package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.auth.AuthConfig
import com.sigmundgranaas.turbo.expressive.core.data.database.PathDao
import com.sigmundgranaas.turbo.expressive.core.data.database.PathEntity
import io.ktor.client.call.body
import io.ktor.client.request.header
import io.ktor.client.request.parameter
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpMethod
import io.ktor.http.HttpStatusCode
import io.ktor.http.contentType
import io.ktor.http.isSuccess
import java.util.UUID
import javax.inject.Inject

/** Server identity returned by a create / update. */
data class RemoteRef(val id: String, val version: Long, val updatedAt: String?)

/** Result of pushing an update (PUT). */
sealed interface TrackUpdateOutcome {
    data class Updated(val ref: RemoteRef) : TrackUpdateOutcome
    /** 412 — adopt the server's authoritative copy. */
    data class Conflict(val server: TrackResponseDto) : TrackUpdateOutcome
    /** 404 — the row is gone server-side; drop it locally. */
    data object Gone : TrackUpdateOutcome
}

/** The network seam for tracks — faked in tests so the syncer logic runs without Ktor. */
interface TrackRemote {
    suspend fun pull(since: String?): TracksDeltaDto
    suspend fun create(row: PathEntity): RemoteRef
    suspend fun update(row: PathEntity): TrackUpdateOutcome
    suspend fun delete(remoteId: String, version: Long)
    /** Fetch a single track by its server id (e.g. one shared with us), or null. */
    suspend fun fetchById(remoteId: String): TrackResponseDto?
}

class TrackSyncApi @Inject constructor(
    private val http: AuthorizedHttp,
) : TrackRemote {
    private val base = "${AuthConfig.BASE_URL}/api/tracks/Tracks"

    override suspend fun pull(since: String?): TracksDeltaDto {
        val resp = http.request(base) {
            method = HttpMethod.Get
            if (!since.isNullOrBlank()) parameter("since", since)
            parameter("limit", PAGE_LIMIT)
        }
        // Non-2xx → empty page so the cursor doesn't advance past unsynced data.
        return if (resp.status.isSuccess()) resp.body() else TracksDeltaDto()
    }

    override suspend fun create(row: PathEntity): RemoteRef {
        val resp = http.request(base) {
            method = HttpMethod.Post
            contentType(ContentType.Application.Json)
            setBody(row.toWriteRequest())
        }
        val dto: TrackResponseDto = resp.body()
        return RemoteRef(dto.id, dto.version, dto.updatedAt)
    }

    override suspend fun update(row: PathEntity): TrackUpdateOutcome {
        val resp = http.request("$base/${row.remoteId}") {
            method = HttpMethod.Put
            header(HttpHeaders.IfMatch, ifMatch(row.version))
            contentType(ContentType.Application.Json)
            setBody(row.toWriteRequest())
        }
        return when {
            resp.status == HttpStatusCode.PreconditionFailed ->
                resp.body<TrackConflictDto>().current?.let { TrackUpdateOutcome.Conflict(it) }
                    ?: TrackUpdateOutcome.Gone
            resp.status == HttpStatusCode.NotFound -> TrackUpdateOutcome.Gone
            resp.status.isSuccess() -> {
                val dto: TrackResponseDto = resp.body()
                TrackUpdateOutcome.Updated(RemoteRef(dto.id, dto.version, dto.updatedAt))
            }
            else -> TrackUpdateOutcome.Gone
        }
    }

    override suspend fun delete(remoteId: String, version: Long) {
        // Any terminal status is fine — the row is removed locally regardless.
        http.request("$base/$remoteId") {
            method = HttpMethod.Delete
            header(HttpHeaders.IfMatch, ifMatch(version))
        }
    }

    override suspend fun fetchById(remoteId: String): TrackResponseDto? {
        val resp = http.request("$base/$remoteId") { method = HttpMethod.Get }
        return if (resp.status.isSuccess()) resp.body() else null
    }

    private companion object {
        const val PAGE_LIMIT = 500
        fun ifMatch(version: Long?) = "\"${version ?: 0}\""
    }
}

/**
 * Pulls track deltas, merges with the local Room rows under [SyncDecisions], then
 * pushes local pending changes. Mirrors the Flutter sync flow.
 */
class TrackSyncer @Inject constructor(
    private val remote: TrackRemote,
    private val dao: PathDao,
) : DomainSyncer {

    override val cursorKey = "tracks"

    override suspend fun sync(since: String?): String? {
        val page = remote.pull(since)
        page.items.forEach { mergeRemote(it) }
        page.deleted.forEach { mergeTombstone(it) }
        pushPending()
        return page.serverTime
    }

    private suspend fun mergeRemote(dto: TrackResponseDto) {
        val local = dao.byRemoteId(dto.id)
        val remoteMs = Iso8601.toEpochMs(dto.updatedAt) ?: 0L
        if (SyncDecisions.pull(local?.toLocalState(), remoteMs) == PullMerge.TakeRemote) {
            dao.upsert(dto.toEntity(local?.id ?: UUID.randomUUID().toString()))
        }
    }

    private suspend fun mergeTombstone(t: TombstoneDto) {
        val local = dao.byRemoteId(t.id) ?: return
        val deletedMs = Iso8601.toEpochMs(t.deletedAt) ?: Long.MAX_VALUE
        when (SyncDecisions.tombstone(local.toLocalState(), deletedMs)) {
            TombstoneMerge.PurgeLocal -> dao.delete(local.id)
            // The user revived it after the server delete: detach from the dead server
            // row so the push pass re-creates it.
            TombstoneMerge.KeepLocal ->
                dao.upsert(local.copy(remoteId = null, version = null, dirty = true))
        }
    }

    private suspend fun pushPending() {
        dao.pendingSync().forEach { row ->
            when (SyncDecisions.push(row.toLocalState())) {
                PushAction.Create -> {
                    val ref = remote.create(row)
                    dao.markSynced(row.id, ref.id, ref.version, Iso8601.toEpochMs(ref.updatedAt) ?: row.updatedAtEpochMs ?: 0L)
                }
                PushAction.Update -> when (val out = remote.update(row)) {
                    is TrackUpdateOutcome.Updated ->
                        dao.markSynced(row.id, out.ref.id, out.ref.version, Iso8601.toEpochMs(out.ref.updatedAt) ?: row.updatedAtEpochMs ?: 0L)
                    is TrackUpdateOutcome.Conflict -> dao.upsert(out.server.toEntity(row.id)) // adopt server, dirty=false
                    TrackUpdateOutcome.Gone -> dao.delete(row.id)
                }
                PushAction.DeleteRemote -> {
                    remote.delete(row.remoteId!!, row.version ?: 0L)
                    dao.delete(row.id)
                }
                PushAction.PurgeLocalOnly -> dao.delete(row.id)
                PushAction.Skip -> Unit
            }
        }
    }
}

// ─────────────────────── PathEntity ⇄ Track wire mappers ───────────────────────

internal fun PathEntity.toLocalState() = LocalState(
    exists = true,
    dirty = dirty,
    updatedAtEpochMs = updatedAtEpochMs,
    hasRemoteId = remoteId != null,
    isTombstone = deletedAtEpochMs != null,
)

internal fun PathEntity.toWriteRequest(): TrackWriteRequest {
    val latLng = decodeLatLng(points)
    return TrackWriteRequest(
        geometry = TrackGeometryDto(
            points = latLng.map { (lat, lng) -> WirePoint(longitude = lng, latitude = lat) },
            elevations = decodeElevationsComplete(elevations, latLng.size),
        ),
        // Carry the full metadata, not just the name — pushing name-only made an
        // Android edit silently DISCARD a colour/style set on another client.
        metadata = TrackMetadataDto(
            name = name,
            colorHex = colorHex,
            iconKey = iconKey,
            lineStyleKey = lineStyleKey,
        ),
        stats = TrackStatsDto(
            distanceMeters = distanceM,
            ascentMeters = ascentM,
            descentMeters = descentM,
            movingTimeSeconds = durationSec,
            recordedAt = if (createdAtEpochMs > 0) Iso8601.fromEpochMs(createdAtEpochMs) else null,
        ),
    )
}

internal fun TrackResponseDto.toEntity(localId: String, readOnly: Boolean = false): PathEntity {
    val latLng = geometry.points.map { it.latitude to it.longitude }
    return PathEntity(
        id = localId,
        name = metadata.name?.takeIf { it.isNotBlank() } ?: "Track",
        source = "Saved",
        points = encodeLatLng(latLng),
        distanceM = stats?.distanceMeters ?: 0.0,
        ascentM = stats?.ascentMeters,
        descentM = stats?.descentMeters,
        durationSec = stats?.movingTimeSeconds,
        createdAtEpochMs = Iso8601.toEpochMs(stats?.recordedAt) ?: Iso8601.toEpochMs(createdAt) ?: 0L,
        elevations = geometry.elevations?.takeIf { it.isNotEmpty() }?.joinToString(";") { it.toString() },
        activityKind = null,
        colorHex = metadata.colorHex,
        iconKey = metadata.iconKey,
        lineStyleKey = metadata.lineStyleKey,
        remoteId = id,
        version = version,
        updatedAtEpochMs = Iso8601.toEpochMs(updatedAt),
        deletedAtEpochMs = null,
        dirty = false,
        readOnly = readOnly,
    )
}

/** "lat,lng;lat,lng" → list of (lat, lng). */
private fun decodeLatLng(encoded: String): List<Pair<Double, Double>> =
    if (encoded.isBlank()) emptyList() else encoded.split(";").mapNotNull { pair ->
        val parts = pair.split(",")
        val lat = parts.getOrNull(0)?.toDoubleOrNull()
        val lng = parts.getOrNull(1)?.toDoubleOrNull()
        if (lat != null && lng != null) lat to lng else null
    }

private fun encodeLatLng(points: List<Pair<Double, Double>>): String =
    points.joinToString(";") { "${it.first},${it.second}" }

/** Only send elevations when every point has one (the wire array is non-nullable doubles). */
private fun decodeElevationsComplete(encoded: String?, pointCount: Int): List<Double>? {
    val parts = encoded?.takeIf { it.isNotBlank() }?.split(";") ?: return null
    if (parts.size != pointCount) return null
    val values = parts.map { it.toDoubleOrNull() }
    return if (values.all { it != null }) values.filterNotNull() else null
}
