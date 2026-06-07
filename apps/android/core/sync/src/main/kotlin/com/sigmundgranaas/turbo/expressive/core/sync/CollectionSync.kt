package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.auth.AuthConfig
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionDao
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionEntity
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

/** Result of pushing a collection update (PUT). */
sealed interface CollectionUpdateOutcome {
    data class Updated(val ref: RemoteRef) : CollectionUpdateOutcome
    data class Conflict(val server: CollectionResponseDto) : CollectionUpdateOutcome
    data object Gone : CollectionUpdateOutcome
}

/** Network seam for collections — faked in tests. */
interface CollectionRemote {
    suspend fun pull(since: String?): CollectionsDeltaDto
    suspend fun create(row: CollectionEntity): RemoteRef
    suspend fun update(row: CollectionEntity): CollectionUpdateOutcome
    suspend fun delete(remoteId: String, version: Long)
}

class CollectionSyncApi @Inject constructor(
    private val http: AuthorizedHttp,
) : CollectionRemote {
    private val base = "${AuthConfig.BASE_URL}/api/collections/Collections"

    override suspend fun pull(since: String?): CollectionsDeltaDto {
        val resp = http.request(base) {
            method = HttpMethod.Get
            if (!since.isNullOrBlank()) parameter("since", since)
            parameter("limit", PAGE_LIMIT)
        }
        return if (resp.status.isSuccess()) resp.body() else CollectionsDeltaDto()
    }

    override suspend fun create(row: CollectionEntity): RemoteRef {
        val resp = http.request(base) {
            method = HttpMethod.Post
            contentType(ContentType.Application.Json)
            setBody(row.toWriteRequest())
        }
        val dto: CollectionResponseDto = resp.body()
        return RemoteRef(dto.id, dto.version, dto.updatedAt)
    }

    override suspend fun update(row: CollectionEntity): CollectionUpdateOutcome {
        val resp = http.request("$base/${row.remoteId}") {
            method = HttpMethod.Put
            header(HttpHeaders.IfMatch, "\"${row.version ?: 0}\"")
            contentType(ContentType.Application.Json)
            setBody(row.toWriteRequest())
        }
        return when {
            resp.status == HttpStatusCode.PreconditionFailed ->
                resp.body<CollectionConflictDto>().current?.let { CollectionUpdateOutcome.Conflict(it) }
                    ?: CollectionUpdateOutcome.Gone
            resp.status == HttpStatusCode.NotFound -> CollectionUpdateOutcome.Gone
            resp.status.isSuccess() -> {
                val dto: CollectionResponseDto = resp.body()
                CollectionUpdateOutcome.Updated(RemoteRef(dto.id, dto.version, dto.updatedAt))
            }
            else -> CollectionUpdateOutcome.Gone
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

/**
 * Syncs collection **metadata** (name / colour / icon). Membership (the embedded
 * `items[]`, managed via the server's separate `/items` endpoints) is not synced
 * yet — the local `collection_item` table has no per-row version tracking. That's
 * a documented follow-on; metadata round-trips fully here.
 */
class CollectionSyncer @Inject constructor(
    private val remote: CollectionRemote,
    private val dao: CollectionDao,
) : DomainSyncer {

    override val cursorKey = "collections"

    override suspend fun sync(since: String?): String? {
        val page = remote.pull(since)
        page.items.forEach { mergeRemote(it) }
        page.deleted.forEach { mergeTombstone(it) }
        pushPending()
        return page.serverTime
    }

    private suspend fun mergeRemote(dto: CollectionResponseDto) {
        val local = dao.byRemoteId(dto.id)
        val remoteMs = Iso8601.toEpochMs(dto.updatedAt) ?: 0L
        if (SyncDecisions.pull(local?.toLocalState(), remoteMs) == PullMerge.TakeRemote) {
            dao.upsert(dto.toEntity(local?.id ?: UUID.randomUUID().toString(), local?.createdAtEpochMs))
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
                    is CollectionUpdateOutcome.Updated ->
                        dao.markSynced(row.id, out.ref.id, out.ref.version, Iso8601.toEpochMs(out.ref.updatedAt) ?: row.updatedAtEpochMs ?: 0L)
                    is CollectionUpdateOutcome.Conflict -> dao.upsert(out.server.toEntity(row.id, row.createdAtEpochMs))
                    CollectionUpdateOutcome.Gone -> dao.delete(row.id)
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

// ─────────────────────── CollectionEntity ⇄ wire mappers ───────────────────────

internal fun CollectionEntity.toLocalState() = LocalState(
    exists = true,
    dirty = dirty,
    updatedAtEpochMs = updatedAtEpochMs,
    hasRemoteId = remoteId != null,
    isTombstone = deletedAtEpochMs != null,
)

internal fun CollectionEntity.toWriteRequest() = CollectionWriteRequest(
    name = name,
    colorHex = colorArgb?.toColorHex(),
    iconKey = icon,
)

internal fun CollectionResponseDto.toEntity(localId: String, existingCreatedAt: Long?) = CollectionEntity(
    id = localId,
    name = name?.takeIf { it.isNotBlank() } ?: "Collection",
    colorArgb = colorHex?.parseColorArgb(),
    icon = iconKey,
    createdAtEpochMs = Iso8601.toEpochMs(createdAt) ?: existingCreatedAt ?: 0L,
    remoteId = id,
    version = version,
    updatedAtEpochMs = Iso8601.toEpochMs(updatedAt),
    deletedAtEpochMs = null,
    dirty = false,
)

/** ARGB long → "#RRGGBB" (alpha dropped — the server stores no alpha). */
internal fun Long.toColorHex(): String = "#%06X".format(this and 0xFFFFFF)

/** "#RRGGBB" → opaque ARGB long; null if unparseable. */
internal fun String.parseColorArgb(): Long? =
    trimStart('#').toLongOrNull(16)?.let { 0xFF000000L or it }
