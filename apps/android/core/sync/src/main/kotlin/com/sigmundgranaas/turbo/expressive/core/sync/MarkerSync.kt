package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.auth.AuthConfig
import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerDao
import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerEntity
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

/** Result of pushing a marker update (PUT). */
sealed interface LocationUpdateOutcome {
    data class Updated(val ref: RemoteRef) : LocationUpdateOutcome
    data class Conflict(val server: LocationResponseDto) : LocationUpdateOutcome
    data object Gone : LocationUpdateOutcome
}

/** Network seam for markers — faked in tests. */
interface LocationRemote {
    suspend fun pull(since: String?): LocationsDeltaDto
    suspend fun create(row: MarkerEntity): RemoteRef
    suspend fun update(row: MarkerEntity): LocationUpdateOutcome
    suspend fun delete(remoteId: String, version: Long)
}

class LocationSyncApi @Inject constructor(
    private val http: AuthorizedHttp,
) : LocationRemote {
    private val base = "${AuthConfig.BASE_URL}/api/geo/locations"

    override suspend fun pull(since: String?): LocationsDeltaDto {
        val resp = http.request(base) {
            method = HttpMethod.Get
            if (!since.isNullOrBlank()) parameter("since", since)
            parameter("limit", PAGE_LIMIT)
        }
        return if (resp.status.isSuccess()) resp.body() else LocationsDeltaDto()
    }

    override suspend fun create(row: MarkerEntity): RemoteRef {
        val resp = http.request(base) {
            method = HttpMethod.Post
            contentType(ContentType.Application.Json)
            setBody(row.toWriteRequest())
        }
        val dto: LocationResponseDto = resp.body()
        return RemoteRef(dto.id, dto.version, dto.updatedAt)
    }

    override suspend fun update(row: MarkerEntity): LocationUpdateOutcome {
        val resp = http.request("$base/${row.remoteId}") {
            method = HttpMethod.Put
            header(HttpHeaders.IfMatch, "\"${row.version ?: 0}\"")
            contentType(ContentType.Application.Json)
            setBody(row.toWriteRequest())
        }
        return when {
            resp.status == HttpStatusCode.PreconditionFailed ->
                resp.body<LocationConflictDto>().current?.let { LocationUpdateOutcome.Conflict(it) }
                    ?: LocationUpdateOutcome.Gone
            resp.status == HttpStatusCode.NotFound -> LocationUpdateOutcome.Gone
            resp.status.isSuccess() -> {
                val dto: LocationResponseDto = resp.body()
                LocationUpdateOutcome.Updated(RemoteRef(dto.id, dto.version, dto.updatedAt))
            }
            else -> LocationUpdateOutcome.Gone
        }
    }

    override suspend fun delete(remoteId: String, version: Long) {
        http.request("$base/$remoteId") {
            method = HttpMethod.Delete
            header(HttpHeaders.IfMatch, "\"$version\"")
        }
    }

    private companion object {
        const val PAGE_LIMIT = 500
    }
}

class MarkerSyncer @Inject constructor(
    private val remote: LocationRemote,
    private val dao: MarkerDao,
) : DomainSyncer {

    override val cursorKey = "geo"

    override suspend fun sync(since: String?): String? {
        val page = remote.pull(since)
        page.items.forEach { mergeRemote(it) }
        page.deleted.forEach { mergeTombstone(it) }
        pushPending()
        return page.serverTime
    }

    private suspend fun mergeRemote(dto: LocationResponseDto) {
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
            TombstoneMerge.KeepLocal -> dao.upsert(local.copy(remoteId = null, version = null, dirty = true))
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
                    is LocationUpdateOutcome.Updated ->
                        dao.markSynced(row.id, out.ref.id, out.ref.version, Iso8601.toEpochMs(out.ref.updatedAt) ?: row.updatedAtEpochMs ?: 0L)
                    is LocationUpdateOutcome.Conflict -> dao.upsert(out.server.toEntity(row.id))
                    LocationUpdateOutcome.Gone -> dao.delete(row.id)
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

// ─────────────────────── MarkerEntity ⇄ Location wire mappers ───────────────────────

internal fun MarkerEntity.toLocalState() = LocalState(
    exists = true,
    dirty = dirty,
    updatedAtEpochMs = updatedAtEpochMs,
    hasRemoteId = remoteId != null,
    isTombstone = deletedAtEpochMs != null,
)

internal fun MarkerEntity.toWriteRequest() = LocationWriteRequest(
    geometry = LocationGeometryDto(longitude = lng, latitude = lat),
    // The server location model carries name/description/icon — colour is local-only.
    display = LocationDisplayDto(name = name, description = notes, icon = kind),
)

internal fun LocationResponseDto.toEntity(localId: String) = MarkerEntity(
    id = localId,
    name = display.name?.takeIf { it.isNotBlank() } ?: "Marker",
    kind = display.icon?.takeIf { it.isNotBlank() } ?: "mountain",
    lat = geometry.latitude,
    lng = geometry.longitude,
    colorArgb = null,
    notes = display.description,
    remoteId = id,
    version = version,
    updatedAtEpochMs = Iso8601.toEpochMs(updatedAt),
    deletedAtEpochMs = null,
    dirty = false,
)
