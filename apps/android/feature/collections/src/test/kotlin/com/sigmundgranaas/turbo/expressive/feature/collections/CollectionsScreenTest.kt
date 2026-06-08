package com.sigmundgranaas.turbo.expressive.feature.collections

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.sigmundgranaas.turbo.expressive.core.data.CollectionRepository
import com.sigmundgranaas.turbo.expressive.domain.CollectionItemType
import com.sigmundgranaas.turbo.expressive.domain.MapCollection
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flowOf
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

private class ScreenCollectionRepository(initial: List<MapCollection> = emptyList()) : CollectionRepository {
    val items = MutableStateFlow(initial)
    override fun observeAll(): Flow<List<MapCollection>> = items
    override suspend fun upsert(collection: MapCollection) {
        items.value = items.value.filterNot { it.id == collection.id } + collection
    }
    override suspend fun delete(id: String) { items.value = items.value.filterNot { it.id == id } }
    override suspend fun addItem(collectionId: String, itemId: String, type: CollectionItemType) = Unit
    override suspend fun removeItem(collectionId: String, itemId: String, type: CollectionItemType) = Unit
    override fun observeItemIds(collectionId: String, type: CollectionItemType): Flow<List<String>> = flowOf(emptyList())
    override fun observeCollectionsForItem(itemId: String, type: CollectionItemType): Flow<List<String>> = flowOf(emptyList())
}

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class CollectionsScreenTest {

    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun `empty repository shows the empty state`() {
        composeRule.setContent {
            CollectionsScreen(onBack = {}, viewModel = CollectionsViewModel(ScreenCollectionRepository()))
        }
        composeRule.onNodeWithText("No collections yet").assertIsDisplayed()
    }

    @Test
    fun `each collection renders its name`() {
        val repo = ScreenCollectionRepository(
            listOf(
                MapCollection(id = "c1", name = "Summer trips", colorArgb = null, itemCount = 3),
                MapCollection(id = "c2", name = "Winter tours", colorArgb = 0xFF1A73E8, itemCount = 0),
            ),
        )
        composeRule.setContent {
            CollectionsScreen(onBack = {}, viewModel = CollectionsViewModel(repo))
        }
        composeRule.onNodeWithText("Summer trips").assertIsDisplayed()
        composeRule.onNodeWithText("Winter tours").assertIsDisplayed()
    }

    @Test
    fun `the create dialog saves a new collection through the view model`() {
        val repo = ScreenCollectionRepository()
        composeRule.setContent {
            CollectionsScreen(onBack = {}, viewModel = CollectionsViewModel(repo))
        }
        // FAB (content description) → editor dialog → Save.
        composeRule.onNodeWithContentDescription("New collection").performClick()
        composeRule.onNodeWithText("Save").performClick()
        composeRule.waitForIdle()
        // A blank name still creates one (defaulted by the view model).
        assert(repo.items.value.size == 1)
    }
}
