package com.sigmundgranaas.turbo.expressive.ui.nav

import androidx.compose.runtime.Composable
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.sigmundgranaas.turbo.expressive.feature.activity.ActivityDetailScreen
import com.sigmundgranaas.turbo.expressive.feature.map.MapScreen
import com.sigmundgranaas.turbo.expressive.feature.recording.RecordingScreen
import com.sigmundgranaas.turbo.expressive.feature.search.SearchScreen
import com.sigmundgranaas.turbo.expressive.feature.settings.SettingsScreen

private object Routes {
    const val MAP = "map"
    const val SEARCH = "search"
    const val SETTINGS = "settings"
    const val RECORDING = "recording"
    const val ACTIVITY = "activity"
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
                onOpenActivityDetail = { nav.navigate(Routes.ACTIVITY) },
            )
        }
        composable(Routes.SEARCH) { SearchScreen(onBack = { nav.popBackStack() }) }
        composable(Routes.SETTINGS) { SettingsScreen(onBack = { nav.popBackStack() }) }
        composable(Routes.RECORDING) { RecordingScreen(onStop = { nav.popBackStack() }) }
        composable(Routes.ACTIVITY) { ActivityDetailScreen(onBack = { nav.popBackStack() }) }
    }
}
