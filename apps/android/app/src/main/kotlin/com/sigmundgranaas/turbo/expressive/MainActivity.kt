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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.sigmundgranaas.turbo.expressive.feature.recording.RecordingService
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import com.sigmundgranaas.turbo.expressive.core.auth.AuthRepository
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

    @Inject lateinit var authRepository: AuthRepository

    /** Set when the Quick Settings tile asked us to begin recording (foreground start). */
    private var autoStartRecording by mutableStateOf(false)

    override fun onResume() {
        super.onResume()
        // Pull/push any changes whenever the app comes to the foreground (no-op when signed out).
        lifecycleScope.launch { syncController.syncNow() }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        if (intent.getBooleanExtra(RecordingService.EXTRA_START_RECORDING, false)) autoStartRecording = true
        handleDeepLink(intent)
    }

    private fun handleDeepLink(intent: Intent?) {
        val data = intent?.takeIf { it.action == Intent.ACTION_VIEW }?.data ?: return
        when {
            data.scheme == "turbo" && data.host == "oauth" -> handleOAuthRedirect(data.getQueryParameter("code"))
            data.pathSegments.firstOrNull() == "link" -> handleShareLink(data.pathSegments.getOrNull(1))
        }
    }

    /** A tapped share link (https://kart.sandring.no/link/{token}) → redeem + pull the resource. */
    private fun handleShareLink(token: String?) {
        val t = token?.takeIf { it.isNotBlank() } ?: return
        lifecycleScope.launch {
            val redeemed = shareLinkRedeemer.redeem(t)
            val msg = if (redeemed is Outcome.Success) R.string.share_link_redeemed else R.string.share_link_failed
            Toast.makeText(this@MainActivity, getString(msg), Toast.LENGTH_LONG).show()
        }
    }

    /** Google OAuth redirect (turbo://oauth?code=…) → finish sign-in with the auth code. */
    private fun handleOAuthRedirect(code: String?) {
        val c = code?.takeIf { it.isNotBlank() } ?: return
        lifecycleScope.launch { authRepository.loginWithGoogle(c) }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        // Branded splash before content; must be installed before super.onCreate().
        installSplashScreen()
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        autoStartRecording = intent?.getBooleanExtra(RecordingService.EXTRA_START_RECORDING, false) == true
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
                        TurboNavGraph(
                            autoStartRecording = autoStartRecording,
                            onAutoStartConsumed = { autoStartRecording = false },
                        )
                    }
                }
            }
        }
        // Cold-start deep link (the warm path is onNewIntent).
        handleDeepLink(intent)
    }
}
