package com.sigmundgranaas.turbo.expressive.core.sync

/**
 * Pure, side-effect-free conflict resolution — the heart of the sync engine,
 * kept free of Room/Ktor so it can be exhaustively unit-tested.
 *
 * Policy (mirrors the Flutter app): pull first, then push. On pull the server
 * wins, **except** a locally-edited row that is newer than the server's copy
 * (the user changed it after the last sync and hasn't uploaded yet). The same
 * rule applies to server deletes: a tombstone wins unless the local row has a
 * newer unsynced edit, in which case we keep it and re-push it as a create.
 */

/** What to do with one remote item during a pull. */
enum class PullMerge { TakeRemote, KeepLocal }

/** What to do with one local row that the server says was deleted. */
enum class TombstoneMerge { PurgeLocal, KeepLocal }

/** What to do with one dirty local row during the push pass. */
enum class PushAction { Create, Update, DeleteRemote, PurgeLocalOnly, Skip }

/** Minimal local-side facts the decisions need; domain content is irrelevant here. */
data class LocalState(
    val exists: Boolean,
    val dirty: Boolean,
    val updatedAtEpochMs: Long?,
    val hasRemoteId: Boolean,
    val isTombstone: Boolean,
)

object SyncDecisions {

    /**
     * Resolve an incoming remote item against the local row.
     * A new remote row (no local match) is always taken.
     */
    fun pull(local: LocalState?, remoteUpdatedAtEpochMs: Long): PullMerge {
        if (local == null || !local.exists) return PullMerge.TakeRemote
        val localNewer = local.dirty &&
            local.updatedAtEpochMs != null &&
            local.updatedAtEpochMs > remoteUpdatedAtEpochMs
        return if (localNewer) PullMerge.KeepLocal else PullMerge.TakeRemote
    }

    /**
     * Resolve a server tombstone against the local row. Keep the local copy only
     * when it has an unsynced edit newer than the delete (the user revived it).
     */
    fun tombstone(local: LocalState?, deletedAtEpochMs: Long): TombstoneMerge {
        if (local == null || !local.exists) return TombstoneMerge.PurgeLocal
        val localRevived = local.dirty &&
            !local.isTombstone &&
            local.updatedAtEpochMs != null &&
            local.updatedAtEpochMs > deletedAtEpochMs
        return if (localRevived) TombstoneMerge.KeepLocal else TombstoneMerge.PurgeLocal
    }

    /** Decide how to push one dirty local row. */
    fun push(local: LocalState): PushAction = when {
        local.isTombstone && local.hasRemoteId -> PushAction.DeleteRemote
        local.isTombstone && !local.hasRemoteId -> PushAction.PurgeLocalOnly
        !local.hasRemoteId -> PushAction.Create
        else -> PushAction.Update
    }
}
