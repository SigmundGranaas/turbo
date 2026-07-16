package com.sigmundgranaas.turbo.expressive.ui.nav

import androidx.compose.animation.AnimatedContentTransitionScope.SlideDirection
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import com.sigmundgranaas.turbo.expressive.core.auth.AuthState
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.feature.auth.AuthScreen
import com.sigmundgranaas.turbo.expressive.feature.auth.AuthViewModel
import com.sigmundgranaas.turbo.expressive.feature.auth.SharingScreen
import com.sigmundgranaas.turbo.expressive.feature.collections.CollectionsScreen
import com.sigmundgranaas.turbo.expressive.feature.map.MapScreen
import com.sigmundgranaas.turbo.expressive.feature.offline.OfflineMapsScreen
import com.sigmundgranaas.turbo.expressive.feature.recording.PathDetailScreen
import com.sigmundgranaas.turbo.expressive.feature.recording.PathsListScreen
import com.sigmundgranaas.turbo.expressive.feature.search.SearchScreen
import com.sigmundgranaas.turbo.expressive.feature.settings.AboutScreen
import com.sigmundgranaas.turbo.expressive.feature.settings.SettingsScreen

private object Routes {
    const val MAP = "map"
    const val SEARCH = "search"
    const val SETTINGS = "settings"
    const val PATHS = "paths"
    const val OFFLINE = "offline"
    const val COLLECTIONS = "collections"
    const val ABOUT = "about"
    const val ACCOUNT = "account"
    const val SHARING = "sharing"
    const val PATH_DETAIL = "path/{pathId}"

    fun pathDetail(id: String) = "path/$id"
}

private const val NAV_MS = 280

@Composable
fun TurboNavGraph(
    autoStartRecording: Boolean = false,
    onAutoStartConsumed: () -> Unit = {},
) {
    val nav = rememberNavController()
    NavHost(
        navController = nav,
        startDestination = Routes.MAP,
        // Clean horizontal push/pop (forward slides in from the right, back slides
        // back out) with a short fade — replaces the default crossfade that made
        // screens look like they were blending into each other.
        enterTransition = { slideIntoContainer(SlideDirection.Start, tween(NAV_MS)) + fadeIn(tween(NAV_MS)) },
        exitTransition = { slideOutOfContainer(SlideDirection.Start, tween(NAV_MS)) + fadeOut(tween(NAV_MS)) },
        popEnterTransition = { slideIntoContainer(SlideDirection.End, tween(NAV_MS)) + fadeIn(tween(NAV_MS)) },
        popExitTransition = { slideOutOfContainer(SlideDirection.End, tween(NAV_MS)) + fadeOut(tween(NAV_MS)) },
    ) {
        composable(Routes.MAP) { entry ->
            val focus by entry.savedStateHandle.getStateFlow<DoubleArray?>("focus", null).collectAsState()
            val showTrack by entry.savedStateHandle.getStateFlow<String?>("showTrack", null).collectAsState()
            val authState by hiltViewModel<AuthViewModel>().state.collectAsState()
            MapScreen(
                onOpenSearch = { nav.navigate(Routes.SEARCH) },
                onOpenSettings = { nav.navigate(Routes.SETTINGS) },
                onOpenPaths = { nav.navigate(Routes.PATHS) },
                onOpenOffline = { nav.navigate(Routes.OFFLINE) },
                onOpenCollections = { nav.navigate(Routes.COLLECTIONS) },
                onOpenAccount = { nav.navigate(Routes.ACCOUNT) },
                accountEmail = (authState as? AuthState.SignedIn)?.account?.email,
                focusRequest = focus?.let { LatLng(it[0], it[1]) },
                onFocusConsumed = { entry.savedStateHandle["focus"] = null },
                showTrackId = showTrack,
                onShowTrackConsumed = { entry.savedStateHandle["showTrack"] = null },
                autoStartRecording = autoStartRecording,
                onAutoStartConsumed = onAutoStartConsumed,
            )
        }
        composable(
            Routes.SEARCH,
            // Search expands/contracts in place (container-transform feel), not a
            // horizontal page slide — the Google-style "search opens over the map".
            enterTransition = { fadeIn(tween(200)) + scaleIn(tween(220), initialScale = 0.94f) },
            exitTransition = { fadeOut(tween(150)) },
            popEnterTransition = { fadeIn(tween(150)) },
            popExitTransition = { fadeOut(tween(200)) + scaleOut(tween(220), targetScale = 0.94f) },
        ) {
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
                onOpenAccount = { nav.navigate(Routes.ACCOUNT) },
            )
        }
        composable(Routes.ABOUT) { AboutScreen(onBack = { nav.popBackStack() }) }
        composable(Routes.ACCOUNT) {
            AuthScreen(
                onBack = { nav.popBackStack() },
                onOpenSharing = { nav.navigate(Routes.SHARING) },
            )
        }
        composable(Routes.SHARING) { SharingScreen(onBack = { nav.popBackStack() }) }
        composable(Routes.OFFLINE) { OfflineMapsScreen(onBack = { nav.popBackStack() }) }
        composable(Routes.COLLECTIONS) { CollectionsScreen(onBack = { nav.popBackStack() }) }
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
