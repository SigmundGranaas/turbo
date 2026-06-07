package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.auth.AuthConfig
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionDao
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionEntity
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionItemEntity
import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerDao
import com.sigmundgranaas.turbo.expressive.core.data.database.PathDao
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
    suspend fun fetchById(remoteId: String): CollectionResponseDto?
    /** Add a resource to a collection (idempotent server-side). */
    suspend fun addItem(collectionRemoteId: String, type: String, uuid: String)
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

    override suspend fun fetchById(remoteId: String): CollectionResponseDto? {
        val resp = http.request("$base/$remoteId") { method = HttpMethod.Get }
        return if (resp.status.isSuccess()) resp.body() else null
    }

    override suspend fun addItem(collectionRemoteId: String, type: String, uuid: String) {
        http.request("$base/$collectionRemoteId/items") {
            method = HttpMethod.Post
            contentType(ContentType.Application.Json)
            setBody(CollectionItemRefDto(type = type, uuid = uuid))
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
    private val pathDao: PathDao,
    private val markerDao: MarkerDao,
) : DomainSyncer {

    override val cursorKey = "collections"

    override suspend fun sync(since: String?): String? {
        val page = remote.pull(since)
        page.items.forEach { mergeRemote(it) }
        page.deleted.forEach { mergeTombstone(it) }
        pushPending()
        pushMembership()
        return page.serverTime
    }

    private suspend fun mergeRemote(dto: CollectionResponseDto) {
        val local = dao.byRemoteId(dto.id)
        val remoteMs = Iso8601.toEpochMs(dto.updatedAt) ?: 0L
        val localId = local?.id ?: UUID.randomUUID().toString()
        if (SyncDecisions.pull(local?.toLocalState(), remoteMs) == PullMerge.TakeRemote) {
            dao.upsert(dto.toEntity(localId, local?.createdAtEpochMs))
        }
        adoptIncomingItems(dto, localId)
    }

    /** Add server memberships locally (never removes — removals are a follow-on). */
    private suspend fun adoptIncomingItems(dto: CollectionResponseDto, localCollectionId: String) {
        dto.items.forEach { ref ->
            val resourceLocalId = when (ref.type.lowercase()) {
                WIRE_TRACK -> pathDao.byRemoteId(ref.uuid)?.id
                WIRE_LOCATION -> markerDao.byRemoteId(ref.uuid)?.id
                else -> null
            } ?: return@forEach
            dao.addItem(CollectionItemEntity(localCollectionId, resourceLocalId, ref.type.toLocalItemType()))
        }
    }

    /** Push each synced collection's local memberships to the server (idempotent POST /items). */
    private suspend fun pushMembership() {
        dao.syncedCollections().forEach { collection ->
            val remoteId = collection.remoteId ?: return@forEach
            dao.itemsForCollection(collection.id).forEach { item ->
                val wireType = item.itemType.toWireItemType() ?: return@forEach
                val resourceRemoteId = when (item.itemType) {
                    LOCAL_PATH -> pathDao.byId(item.itemId)?.remoteId
                    LOCAL_MARKER -> markerDao.byId(item.itemId)?.remoteId
                    else -> null
                } ?: return@forEach // resource not synced yet — retry next sync
                remote.addItem(remoteId, wireType, resourceRemoteId)
            }
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

// Membership type mapping: local CollectionItemType.name ⇄ server item type.
private const val LOCAL_PATH = "Path"
private const val LOCAL_MARKER = "Marker"
private const val WIRE_TRACK = "track"
private const val WIRE_LOCATION = "location"

private fun String.toWireItemType(): String? = when (this) {
    LOCAL_PATH -> WIRE_TRACK
    LOCAL_MARKER -> WIRE_LOCATION
    else -> null
}

private fun String.toLocalItemType(): String = when (lowercase()) {
    WIRE_TRACK -> LOCAL_PATH
    else -> LOCAL_MARKER
}

/** ARGB long → "#RRGGBB" (alpha dropped — the server stores no alpha). */
internal fun Long.toColorHex(): String = "#%06X".format(this and 0xFFFFFF)

/** "#RRGGBB" → opaque ARGB long; null if unparseable. */
internal fun String.parseColorArgb(): Long? =
    trimStart('#').toLongOrNull(16)?.let { 0xFF000000L or it }
