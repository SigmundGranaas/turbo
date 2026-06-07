package com.sigmundgranaas.turbo.expressive.ui.nav

import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.feature.collections.CollectionsScreen
import com.sigmundgranaas.turbo.expressive.feature.map.MapScreen
import com.sigmundgranaas.turbo.expressive.feature.offline.OfflineMapsScreen
import com.sigmundgranaas.turbo.expressive.feature.recording.PathDetailScreen
import com.sigmundgranaas.turbo.expressive.feature.recording.PathsListScreen
import com.sigmundgranaas.turbo.expressive.feature.recording.RecordingScreen
import com.sigmundgranaas.turbo.expressive.feature.search.SearchScreen
import com.sigmundgranaas.turbo.expressive.feature.settings.AboutScreen
import com.sigmundgranaas.turbo.expressive.feature.settings.SettingsScreen

private object Routes {
    const val MAP = "map"
    const val SEARCH = "search"
    const val SETTINGS = "settings"
    const val RECORDING = "recording"
    const val PATHS = "paths"
    const val OFFLINE = "offline"
    const val COLLECTIONS = "collections"
    const val ABOUT = "about"
    const val PATH_DETAIL = "path/{pathId}"

    fun pathDetail(id: String) = "path/$id"
}

@Composable
fun TurboNavGraph() {
    val nav = rememberNavController()
    NavHost(navController = nav, startDestination = Routes.MAP) {
        composable(Routes.MAP) { entry ->
            val focus by entry.savedStateHandle.getStateFlow<DoubleArray?>("focus", null).collectAsState()
            val showTrack by entry.savedStateHandle.getStateFlow<String?>("showTrack", null).collectAsState()
            MapScreen(
                onOpenSearch = { nav.navigate(Routes.SEARCH) },
                onOpenSettings = { nav.navigate(Routes.SETTINGS) },
                onOpenRecording = { nav.navigate(Routes.RECORDING) },
                onOpenPaths = { nav.navigate(Routes.PATHS) },
                onOpenOffline = { nav.navigate(Routes.OFFLINE) },
                onOpenCollections = { nav.navigate(Routes.COLLECTIONS) },
                focusRequest = focus?.let { LatLng(it[0], it[1]) },
                onFocusConsumed = { entry.savedStateHandle["focus"] = null },
                showTrackId = showTrack,
                onShowTrackConsumed = { entry.savedStateHandle["showTrack"] = null },
            )
        }
        composable(Routes.SEARCH) {
            SearchScreen(
                onBack = { nav.popBackStack() },
                onPick = { lat, lng, _ ->
                    nav.getBackStackEntry(Routes.MAP).savedStateHandle["focus"] = doubleArrayOf(lat, lng)
                    nav.popBackStack()
                },
            )
        }
        composable(Routes.SETTINGS) {
            SettingsScreen(
                onBack = { nav.popBackStack() },
                onOpenAbout = { nav.navigate(Routes.ABOUT) },
            )
        }
        composable(Routes.ABOUT) { AboutScreen(onBack = { nav.popBackStack() }) }
        composable(Routes.OFFLINE) { OfflineMapsScreen(onBack = { nav.popBackStack() }) }
        composable(Routes.COLLECTIONS) { CollectionsScreen(onBack = { nav.popBackStack() }) }
        composable(Routes.RECORDING) { RecordingScreen(onStop = { nav.popBackStack() }) }
        composable(Routes.PATHS) {
            PathsListScreen(
                onBack = { nav.popBackStack() },
                onOpen = { id -> nav.navigate(Routes.pathDetail(id)) },
            )
        }
        composable(
            Routes.PATH_DETAIL,
            arguments = listOf(navArgument("pathId") { type = NavType.StringType }),
        ) { entry ->
            PathDetailScreen(
                pathId = entry.arguments?.getString("pathId").orEmpty(),
                onBack = { nav.popBackStack() },
                onShowOnMap = { id ->
                    nav.getBackStackEntry(Routes.MAP).savedStateHandle["showTrack"] = id
                    nav.popBackStack(Routes.MAP, inclusive = false)
                },
            )
        }
    }
}
