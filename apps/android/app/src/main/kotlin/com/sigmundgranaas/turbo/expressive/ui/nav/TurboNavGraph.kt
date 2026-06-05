package com.sigmundgranaas.turbo.expressive.ui.nav

import androidx.compose.runtime.Composable
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.feature.activity.ActivitiesHubScreen
import com.sigmundgranaas.turbo.expressive.feature.activity.ActivityDetailScreen
import com.sigmundgranaas.turbo.expressive.feature.activity.PathDetailScreen
import com.sigmundgranaas.turbo.expressive.feature.activity.PathsListScreen
import com.sigmundgranaas.turbo.expressive.feature.map.MapScreen
import com.sigmundgranaas.turbo.expressive.feature.offline.OfflineMapsScreen
import com.sigmundgranaas.turbo.expressive.feature.recording.RecordingScreen
import com.sigmundgranaas.turbo.expressive.feature.search.SearchScreen
import com.sigmundgranaas.turbo.expressive.feature.settings.SettingsScreen

private object Routes {
    const val MAP = "map"
    const val SEARCH = "search"
    const val SETTINGS = "settings"
    const val RECORDING = "recording"
    const val ACTIVITIES = "activities"
    const val PATHS = "paths"
    const val OFFLINE = "offline"
    const val ACTIVITY_DETAIL = "activity/{kind}"
    const val PATH_DETAIL = "path/{pathId}"

    fun activityDetail(kind: ActivityKindId) = "activity/${kind.name}"
    fun pathDetail(id: String) = "path/$id"
}

@Composable
fun TurboNavGraph() {
    val nav = rememberNavController()
    NavHost(navController = nav, startDestination = Routes.MAP) {
        composable(Routes.MAP) {
            MapScreen(
                onOpenSearch = { nav.navigate(Routes.SEARCH) },
                onOpenSettings = { nav.navigate(Routes.SETTINGS) },
                onOpenRecording = { nav.navigate(Routes.RECORDING) },
                onOpenPaths = { nav.navigate(Routes.PATHS) },
                onOpenActivities = { nav.navigate(Routes.ACTIVITIES) },
                onOpenOffline = { nav.navigate(Routes.OFFLINE) },
            )
        }
        composable(Routes.SEARCH) { SearchScreen(onBack = { nav.popBackStack() }) }
        composable(Routes.SETTINGS) { SettingsScreen(onBack = { nav.popBackStack() }) }
        composable(Routes.RECORDING) { RecordingScreen(onStop = { nav.popBackStack() }) }
        composable(Routes.OFFLINE) { OfflineMapsScreen(onBack = { nav.popBackStack() }) }
        composable(Routes.PATHS) {
            PathsListScreen(
                onBack = { nav.popBackStack() },
                onOpen = { id -> nav.navigate(Routes.pathDetail(id)) },
            )
        }
        composable(Routes.ACTIVITIES) {
            ActivitiesHubScreen(
                onBack = { nav.popBackStack() },
                onOpen = { kind -> nav.navigate(Routes.activityDetail(kind)) },
            )
        }
        composable(
            Routes.ACTIVITY_DETAIL,
            arguments = listOf(navArgument("kind") { type = NavType.StringType }),
        ) { entry ->
            val kindName = entry.arguments?.getString("kind")
            val kind = ActivityKindId.entries.firstOrNull { it.name == kindName } ?: ActivityKindId.Skiing
            ActivityDetailScreen(onBack = { nav.popBackStack() }, kind = kind)
        }
        composable(
            Routes.PATH_DETAIL,
            arguments = listOf(navArgument("pathId") { type = NavType.StringType }),
        ) { entry ->
            PathDetailScreen(pathId = entry.arguments?.getString("pathId").orEmpty(), onBack = { nav.popBackStack() })
        }
    }
}
