package com.sigmundgranaas.turbo.expressive.feature.settings

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.OpenInNew
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.components.SpecRow
import com.sigmundgranaas.turbo.expressive.ui.components.TurboCard

private const val PRIVACY_URL = "https://kart.sandring.no/privacy"
private const val TERMS_URL = "https://kart.sandring.no/terms"

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AboutScreen(onBack: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    val context = LocalContext.current
    val version = remember {
        runCatching {
            val pkg = context.packageManager.getPackageInfo(context.packageName, 0)
            "${pkg.versionName} (${pkg.longVersionCode})"
        }.getOrNull() ?: "—"
    }

    Scaffold(
        containerColor = cs.surface,
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.settings_about)) },
                navigationIcon = {
                    IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.action_back)) }
                },
            )
        },
    ) { inner ->
        Column(
            Modifier.fillMaxSize().padding(inner).verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
        ) {
            Spacer(Modifier.height(8.dp))
            Text("Turbo", style = MaterialTheme.typography.headlineMedium, color = cs.onSurface)
            Text(stringResource(R.string.about_tagline), style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
            Spacer(Modifier.height(4.dp))
            Text(stringResource(R.string.about_version, version), style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)

            Spacer(Modifier.height(20.dp))
            SectionLabel(stringResource(R.string.about_section_map))
            Spacer(Modifier.height(8.dp))
            TurboCard(Modifier.fillMaxWidth()) {
                SpecRow("Topographic", "© Kartverket")
                SpecRow("Streets", "© OpenStreetMap contributors")
                SpecRow("Satellite", "© Esri / Kartverket")
                SpecRow("Rendering", "Turbomap (wgpu / Rust)")
                SpecRow("Routing", "kart-api.sandring.no")
            }

            Spacer(Modifier.height(16.dp))
            SectionLabel(stringResource(R.string.about_section_conditions))
            Spacer(Modifier.height(8.dp))
            TurboCard(Modifier.fillMaxWidth()) {
                SpecRow("Weather", "© MET Norway (Yr / api.met.no)")
                SpecRow("Avalanche", "© Varsom / NVE")
                SpecRow("Weather icons", "met.no weathericons (MIT)")
            }

            Spacer(Modifier.height(16.dp))
            SectionLabel(stringResource(R.string.about_section_open_source))
            Spacer(Modifier.height(8.dp))
            TurboCard(Modifier.fillMaxWidth()) {
                SpecRow("UI", "Jetpack Compose · Material 3 (Apache-2.0)")
                SpecRow("DI", "Hilt / Dagger (Apache-2.0)")
                SpecRow("Networking", "Ktor · OkHttp (Apache-2.0)")
                SpecRow("Storage", "AndroidX Room · DataStore (Apache-2.0)")
                SpecRow("Maps", "Turbomap · wgpu (Apache-2.0 / MIT)")
            }

            Spacer(Modifier.height(16.dp))
            SectionLabel(stringResource(R.string.about_section_legal))
            Spacer(Modifier.height(8.dp))
            TurboCard(Modifier.fillMaxWidth()) {
                LinkRow(stringResource(R.string.about_privacy)) { context.openUrl(PRIVACY_URL) }
                LinkRow(stringResource(R.string.about_terms)) { context.openUrl(TERMS_URL) }
            }
            Spacer(Modifier.height(28.dp))
        }
    }
}

@Composable
private fun LinkRow(label: String, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().clickable(onClick = onClick).padding(vertical = 12.dp),
    ) {
        Text(label, style = MaterialTheme.typography.bodyLarge, color = cs.onSurface, modifier = Modifier.weight(1f))
        Icon(Icons.AutoMirrored.Rounded.OpenInNew, null, tint = cs.onSurfaceVariant)
    }
}

private fun android.content.Context.openUrl(url: String) {
    runCatching { startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url))) }
}
