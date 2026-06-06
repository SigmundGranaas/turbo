package com.sigmundgranaas.turbo.expressive.feature.collections

import com.sigmundgranaas.turbo.expressive.core.data.CollectionRepository
import com.sigmundgranaas.turbo.expressive.domain.CollectionItemType
import com.sigmundgranaas.turbo.expressive.domain.MapCollection
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

private class FakeCollectionRepository : CollectionRepository {
    val items = MutableStateFlow<List<MapCollection>>(emptyList())
    override fun observeAll(): Flow<List<MapCollection>> = items
    override suspend fun upsert(collection: MapCollection) {
        items.value = items.value.filterNot { it.id == collection.id } + collection
    }
    override suspend fun delete(id: String) { items.value = items.value.filterNot { it.id == id } }
    override suspend fun addItem(collectionId: String, itemId: String, type: CollectionItemType) = Unit
    override suspend fun removeItem(collectionId: String, itemId: String, type: CollectionItemType) = Unit
    override fun observeItemIds(collectionId: String, type: CollectionItemType): Flow<List<String>> = flowOf(emptyList())
}

@OptIn(ExperimentalCoroutinesApi::class)
class CollectionsViewModelTest {

    @Before fun setUp() = Dispatchers.setMain(kotlinx.coroutines.test.StandardTestDispatcher())
    @After fun tearDown() = Dispatchers.resetMain()

    @Test
    fun `upsert with null id creates a collection`() = runTest {
        val repo = FakeCollectionRepository()
        val vm = CollectionsViewModel(repo)
        vm.upsert(id = null, name = "Summer trips", colorArgb = 0xFF1A73E8)
        advanceUntilIdle()
        assertEquals(1, repo.items.value.size)
        assertEquals("Summer trips", repo.items.value[0].name)
    }

    @Test
    fun `blank name falls back to a default`() = runTest {
        val repo = FakeCollectionRepository()
        val vm = CollectionsViewModel(repo)
        vm.upsert(id = null, name = "   ", colorArgb = null)
        advanceUntilIdle()
        assertEquals("Collection", repo.items.value[0].name)
    }

    @Test
    fun `upsert with an existing id renames in place and delete removes`() = runTest {
        val repo = FakeCollectionRepository()
        repo.items.value = listOf(MapCollection("c1", "Old", null, null, 0))
        val vm = CollectionsViewModel(repo)
        vm.upsert(id = "c1", name = "New", colorArgb = null); advanceUntilIdle()
        assertEquals(listOf("New"), repo.items.value.map { it.name })
        vm.delete("c1"); advanceUntilIdle()
        assertTrue(repo.items.value.isEmpty())
    }
}
