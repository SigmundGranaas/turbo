package com.sigmundgranaas.turbo.expressive.feature.offline

import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * What the foreground service counts as work.
 *
 * These two predicates decide whether [OfflineDownloadService] stays
 * alive and whether its Pause button appears. They used to be three
 * hand-written status lists that had drifted: a device pack build runs
 * *before* the first tile and can take minutes, and the list guarding
 * teardown had never learned about [OfflineStatus.Building] — so a build
 * would start, the 15-second grace timer would find no work, and the
 * service would stop out from under it.
 */
class OfflineWorkStatusTest {

    @Test
    fun `a build counts as pending work, or the service stops during it`() {
        assertTrue(OfflineStatus.Building.isPendingWork)
        assertTrue(OfflineStatus.Downloading.isPendingWork)
        // Paused too: the notification's Resume button is the way back.
        assertTrue(OfflineStatus.Paused.isPendingWork)
        assertFalse(OfflineStatus.Complete.isPendingWork)
        assertFalse(OfflineStatus.Failed.isPendingWork)
    }

    @Test
    fun `active work is what Pause acts on`() {
        assertTrue(OfflineStatus.Building.isActiveWork)
        assertTrue(OfflineStatus.Downloading.isActiveWork)
        assertFalse("already paused", OfflineStatus.Paused.isActiveWork)
        assertFalse(OfflineStatus.Complete.isActiveWork)
        assertFalse(OfflineStatus.Failed.isActiveWork)
    }

    /** Active is a subset of pending — a state that is one but not the
     *  other in the wrong direction would be paused-and-pausable. */
    @Test
    fun `everything active is also pending`() {
        val wrong = OfflineStatus.entries.filter { it.isActiveWork && !it.isPendingWork }
        assertEquals(emptyList<OfflineStatus>(), wrong)
    }
}
