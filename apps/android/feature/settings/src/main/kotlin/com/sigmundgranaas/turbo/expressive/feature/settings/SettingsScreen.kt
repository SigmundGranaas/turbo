package com.sigmundgranaas.turbo.expressive.feature.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.ChevronRight
import androidx.compose.material.icons.rounded.Explore
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.Straighten
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.domain.ThemeMode
import com.sigmundgranaas.turbo.expressive.ui.components.Cookie
import com.sigmundgranaas.turbo.expressive.ui.components.ListRowItem
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

@Composable
fun SettingsScreen(
    onBack: () -> Unit,
    onOpenAbout: () -> Unit = {},
    viewModel: SettingsViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val settings by viewModel.state.collectAsStateWithLifecycle()

    Scaffold(containerColor = cs.surface) { inner ->
        Column(
            Modifier.fillMaxSize().padding(inner).verticalScroll(rememberScrollState()),
        ) {
            IconButton(onClick = onBack, modifier = Modifier.padding(start = 4.dp, top = 4.dp)) {
                Icon(Icons.AutoMirrored.Rounded.ArrowBack, "Back", tint = cs.onSurface)
            }
            Text(
                "Settings",
                style = MaterialTheme.typography.headlineLarge,
                color = cs.onSurface,
                modifier = Modifier.padding(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 16.dp),
            )

            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.padding(horizontal = 16.dp).fillMaxWidth()
                    .clip(RoundedCornerShape(TurboRadius.xl)).background(cs.primaryContainer).padding(18.dp),
            ) {
                Cookie(size = 56.dp, fill = cs.surface) { Text("S", style = MaterialTheme.typography.titleLarge, color = cs.onPrimaryContainer) }
                Spacer(Modifier.width(16.dp))
                Column(Modifier.weight(1f)) {
                    Text("Sigmund G.", style = MaterialTheme.typography.titleMedium, color = cs.onPrimaryContainer)
                    Text("sigmund@turkart.no", style = MaterialTheme.typography.bodySmall, color = cs.onPrimaryContainer)
                }
                Icon(Icons.Rounded.ChevronRight, null, tint = cs.onPrimaryContainer)
            }

            Spacer(Modifier.height(14.dp))
            SettingsGroup {
                ListRowItem(
                    Icons.Rounded.Palette, "Appearance",
                    subtitle = when (settings.themeMode) {
                        ThemeMode.System -> "Follow system"
                        ThemeMode.Light -> "Light theme"
                        ThemeMode.Dark -> "Dark theme"
                    },
                )
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.padding(bottom = 12.dp),
                ) {
                    ThemeMode.entries.forEach { mode ->
                        FilterChip(
                            selected = settings.themeMode == mode,
                            onClick = { viewModel.setThemeMode(mode) },
                            label = { Text(themeModeLabel(mode)) },
                            modifier = Modifier.testTag("theme_${mode.name}"),
                        )
                    }
                }
            }
            SettingsGroup {
                ListRowItem(
                    Icons.Rounded.Explore, "Compass orientation", subtitle = "Rotate map to heading",
                    trailing = { Switch(settings.compassOrientation, viewModel::setCompass) },
                )
                HorizontalDivider(color = cs.outlineVariant)
                ListRowItem(
                    Icons.Rounded.MyLocation, "Follow My Location",
                    trailing = { Switch(settings.followLocation, viewModel::setFollow) },
                )
                HorizontalDivider(color = cs.outlineVariant)
                ListRowItem(
                    Icons.Rounded.Straighten, "Units", subtitle = if (settings.metricUnits) "Metric · km, m" else "Imperial · mi, ft",
                    trailing = { Switch(settings.metricUnits, viewModel::setMetric, modifier = Modifier.testTag("unitsSwitch")) },
                )
            }
            SettingsGroup {
                ListRowItem(
                    Icons.Rounded.Info, "About",
                    subtitle = "Version, data sources, licenses",
                    trailing = { Icon(Icons.Rounded.ChevronRight, null, tint = cs.onSurfaceVariant) },
                    modifier = Modifier.clickable(onClick = onOpenAbout),
                )
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}

private fun themeModeLabel(mode: ThemeMode): String = when (mode) {
    ThemeMode.System -> "System"
    ThemeMode.Light -> "Light"
    ThemeMode.Dark -> "Dark"
}

@Composable
private fun SettingsGroup(content: @Composable () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Column(
        Modifier.padding(horizontal = 16.dp, vertical = 7.dp).fillMaxWidth()
            .clip(RoundedCornerShape(TurboRadius.xl)).background(cs.surfaceContainerHigh)
            .padding(horizontal = 18.dp, vertical = 4.dp),
    ) { content() }
}
