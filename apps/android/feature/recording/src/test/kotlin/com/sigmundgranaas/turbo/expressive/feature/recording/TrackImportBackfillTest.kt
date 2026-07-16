package com.sigmundgranaas.turbo.expressive.feature.recording

import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test

private class CapturingPathRepo : PathRepository {
    var saved: SavedPath? = null
    override fun observeAll(): Flow<List<SavedPath>> = MutableStateFlow(emptyList())
    override suspend fun byId(id: String): SavedPath? = null
    override suspend fun save(path: SavedPath) { saved = path }
    override suspend fun delete(id: String) = Unit
    override suspend fun remoteId(id: String): String? = null
}

/** Importing a file without elevation data (the user's goal: a working elevation
 *  chart + honest ascent/descent on import): missing per-point elevations are
 *  filled from the DEM; a file that brought its own data keeps it; and a DEM
 *  outage degrades to importing the track unchanged, never failing the import. */
class TrackImportBackfillTest {
    private val dispatcher = StandardTestDispatcher()

    @Before fun setUp() = Dispatchers.setMain(dispatcher)

    @After fun tearDown() = Dispatchers.resetMain()

    private val bare = ParsedTrack(
        name = "Hill walk",
        geo = GeoPath(
            points = listOf(LatLng(69.6, 18.95), LatLng(69.61, 18.96), LatLng(69.62, 18.97)),
            source = GeoPathSource.Saved,
        ),
    )

    @Test
    fun `a track without elevations gets them backfilled and climb recomputed`() = runTest(dispatcher) {
        val repo = CapturingPathRepo()
        val vm = PathsViewModel(repo, NoopSharing, FixedElevations(listOf(100.0, 150.0, 120.0)))
        vm.importTrack(bare, fallbackName = "file")
        dispatcher.scheduler.advanceUntilIdle()

        val saved = repo.saved!!
        assertEquals(listOf(100.0, 150.0, 120.0), saved.path.elevations)
        assertEquals(50.0, saved.path.ascentM!!, 1e-6)
        assertEquals(30.0, saved.path.descentM!!, 1e-6)
    }

    @Test
    fun `a file that brought its own elevations keeps them untouched`() = runTest(dispatcher) {
        val repo = CapturingPathRepo()
        val withEle = bare.copy(geo = bare.geo.copy(elevations = listOf(10.0, 20.0, 30.0), ascentM = 20.0))
        val vm = PathsViewModel(repo, NoopSharing, FixedElevations(listOf(999.0)))
        vm.importTrack(withEle, fallbackName = "file")
        dispatcher.scheduler.advanceUntilIdle()

        assertEquals(listOf(10.0, 20.0, 30.0), repo.saved!!.path.elevations)
        assertEquals(20.0, repo.saved!!.path.ascentM!!, 1e-6)
    }

    @Test
    fun `a DEM outage imports the track unchanged instead of failing`() = runTest(dispatcher) {
        val repo = CapturingPathRepo()
        val vm = PathsViewModel(repo, NoopSharing, NoElevations)
        vm.importTrack(bare, fallbackName = "file")
        dispatcher.scheduler.advanceUntilIdle()

        assertEquals("Hill walk", repo.saved!!.name)
        assertNull(repo.saved!!.path.elevations)
    }
}
