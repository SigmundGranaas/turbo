package com.sigmundgranaas.turbo.expressive.core.sync

import org.junit.Assert.assertEquals
import org.junit.Test

class SyncDecisionsTest {

    private fun local(
        dirty: Boolean = false,
        updatedAt: Long? = 100,
        hasRemoteId: Boolean = true,
        tombstone: Boolean = false,
    ) = LocalState(exists = true, dirty = dirty, updatedAtEpochMs = updatedAt, hasRemoteId = hasRemoteId, isTombstone = tombstone)

    // ── pull ──

    @Test fun `a brand-new remote item is taken`() {
        assertEquals(PullMerge.TakeRemote, SyncDecisions.pull(local = null, remoteUpdatedAtEpochMs = 50))
    }

    @Test fun `server wins over a clean local row`() {
        assertEquals(PullMerge.TakeRemote, SyncDecisions.pull(local(dirty = false, updatedAt = 999), remoteUpdatedAtEpochMs = 10))
    }

    @Test fun `a newer unsynced local edit beats the server`() {
        assertEquals(PullMerge.KeepLocal, SyncDecisions.pull(local(dirty = true, updatedAt = 200), remoteUpdatedAtEpochMs = 100))
    }

    @Test fun `an older dirty local row still yields to a newer server copy`() {
        assertEquals(PullMerge.TakeRemote, SyncDecisions.pull(local(dirty = true, updatedAt = 50), remoteUpdatedAtEpochMs = 100))
    }

    // ── tombstone ──

    @Test fun `server delete purges a clean local row`() {
        assertEquals(TombstoneMerge.PurgeLocal, SyncDecisions.tombstone(local(dirty = false), deletedAtEpochMs = 100))
    }

    @Test fun `a local revival newer than the delete is kept`() {
        assertEquals(TombstoneMerge.KeepLocal, SyncDecisions.tombstone(local(dirty = true, updatedAt = 200, tombstone = false), deletedAtEpochMs = 100))
    }

    @Test fun `a local tombstone is not treated as a revival`() {
        assertEquals(TombstoneMerge.PurgeLocal, SyncDecisions.tombstone(local(dirty = true, updatedAt = 200, tombstone = true), deletedAtEpochMs = 100))
    }

    @Test fun `no local row means nothing to purge`() {
        assertEquals(TombstoneMerge.PurgeLocal, SyncDecisions.tombstone(local = null, deletedAtEpochMs = 100))
    }

    // ── push ──

    @Test fun `a dirty row with no remote id is created`() {
        assertEquals(PushAction.Create, SyncDecisions.push(local(hasRemoteId = false, tombstone = false)))
    }

    @Test fun `a dirty synced row is updated`() {
        assertEquals(PushAction.Update, SyncDecisions.push(local(hasRemoteId = true, tombstone = false)))
    }

    @Test fun `a synced tombstone is deleted remotely`() {
        assertEquals(PushAction.DeleteRemote, SyncDecisions.push(local(hasRemoteId = true, tombstone = true)))
    }

    @Test fun `an unsynced tombstone is purged locally only`() {
        assertEquals(PushAction.PurgeLocalOnly, SyncDecisions.push(local(hasRemoteId = false, tombstone = true)))
    }
}
