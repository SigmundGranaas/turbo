package com.sigmundgranaas.turbo.expressive

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.core.sync.ShareLinkRedeemer
import com.sigmundgranaas.turbo.expressive.core.sync.SyncController
import kotlinx.coroutines.launch
import com.sigmundgranaas.turbo.expressive.domain.ThemeMode
import com.sigmundgranaas.turbo.expressive.domain.UserSettings
import com.sigmundgranaas.turbo.expressive.ui.nav.TurboNavGraph
import com.sigmundgranaas.turbo.expressive.ui.theme.LocalMetricUnits
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboTheme
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    @Inject lateinit var settingsRepository: SettingsRepository

    @Inject lateinit var syncController: SyncController

    @Inject lateinit var shareLinkRedeemer: ShareLinkRedeemer

    override fun onResume() {
        super.onResume()
        // Pull/push any changes whenever the app comes to the foreground (no-op when signed out).
        lifecycleScope.launch { syncController.syncNow() }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleShareLink(intent)
    }

    /** A tapped share link (https://kart.sandring.no/link/{token}) → redeem + pull the resource. */
    private fun handleShareLink(intent: Intent?) {
        val segments = intent?.takeIf { it.action == Intent.ACTION_VIEW }?.data?.pathSegments ?: return
        if (segments.firstOrNull() != "link") return
        val token = segments.getOrNull(1)?.takeIf { it.isNotBlank() } ?: return
        lifecycleScope.launch {
            val redeemed = shareLinkRedeemer.redeem(token)
            val msg = if (redeemed is Outcome.Success) R.string.share_link_redeemed else R.string.share_link_failed
            Toast.makeText(this@MainActivity, getString(msg), Toast.LENGTH_LONG).show()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        // Branded splash before content; must be installed before super.onCreate().
        installSplashScreen()
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        setContent {
            val settings by settingsRepository.settings.collectAsStateWithLifecycle(UserSettings())
            val dark = when (settings.themeMode) {
                ThemeMode.System -> isSystemInDarkTheme()
                ThemeMode.Light -> false
                ThemeMode.Dark -> true
            }
            TurboTheme(darkTheme = dark) {
                CompositionLocalProvider(LocalMetricUnits provides settings.metricUnits) {
                    Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
                        TurboNavGraph()
                    }
                }
            }
        }
        // Cold-start deep link (the warm path is onNewIntent).
        handleShareLink(intent)
    }
}
