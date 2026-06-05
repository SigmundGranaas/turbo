package com.sigmundgranaas.turbo.expressive.feature.offline

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.CloudDownload
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Map
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.components.TurboCard
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

/** A downloaded (or downloading) offline region. */
data class OfflineRegion(
    val name: String,
    val sizeMb: Int,
    /** null = complete; 0..1 = in-progress fraction. */
    val progress: Float?,
)

private val sampleRegions = listOf(
    OfflineRegion("Tromsø & Kvaløya", 248, null),
    OfflineRegion("Lyngen Alps", 312, null),
    OfflineRegion("Senja", 196, 0.42f),
)

/**
 * Offline maps manager: total storage summary, the list of downloaded/in-flight
 * regions (wavy progress while downloading), and an add-region CTA.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OfflineMapsScreen(onBack: () -> Unit, onAddRegion: () -> Unit = {}) {
    val cs = MaterialTheme.colorScheme
    val totalMb = sampleRegions.filter { it.progress == null }.sumOf { it.sizeMb }
    Scaffold(
        containerColor = cs.surface,
        topBar = {
            TopAppBar(
                title = { Text("Offline maps", style = MaterialTheme.typography.headlineSmall) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, "Back") } },
            )
        },
    ) { pad ->
        LazyColumn(
            Modifier.fillMaxSize().padding(pad).padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item {
                Spacer(Modifier.height(4.dp))
                TurboCard(color = cs.primaryContainer) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Rounded.CloudDownload, null, tint = cs.onPrimaryContainer, modifier = Modifier.size(28.dp))
                        Spacer(Modifier.size(14.dp))
                        Column {
                            Text("$totalMb MB stored", style = MaterialTheme.typography.titleLarge, color = cs.onPrimaryContainer)
                            Text("${sampleRegions.count { it.progress == null }} regions available offline", style = MaterialTheme.typography.bodyMedium, color = cs.onPrimaryContainer)
                        }
                    }
                }
            }
            item { SectionLabel("Regions") }
            items(sampleRegions.size) { i -> RegionRow(sampleRegions[i]) }
            item {
                AddRegionRow(onAddRegion)
                Spacer(Modifier.height(24.dp))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun RegionRow(region: OfflineRegion) {
    val cs = MaterialTheme.colorScheme
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.xl))
            .background(cs.surfaceContainerHigh).padding(14.dp),
    ) {
        Box(Modifier.size(44.dp).clip(RoundedCornerShape(TurboRadius.m)).background(cs.surfaceContainerHighest), contentAlignment = Alignment.Center) {
            Icon(Icons.Rounded.Map, null, tint = cs.primary, modifier = Modifier.size(24.dp))
        }
        Spacer(Modifier.size(14.dp))
        Column(Modifier.weight(1f)) {
            Text(region.name, style = MaterialTheme.typography.titleMedium, color = cs.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (region.progress != null) {
                Spacer(Modifier.height(6.dp))
                LinearWavyProgressIndicator(progress = { region.progress }, modifier = Modifier.fillMaxWidth())
                Text("Downloading · ${(region.progress * 100).toInt()}%", style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
            } else {
                Text("${region.sizeMb} MB", style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
            }
        }
        Spacer(Modifier.size(8.dp))
        if (region.progress == null) {
            IconButton(onClick = {}) { Icon(Icons.Rounded.DeleteOutline, "Remove", tint = cs.onSurfaceVariant) }
        } else {
            Icon(Icons.Rounded.CheckCircle, null, tint = cs.primary, modifier = Modifier.size(22.dp))
        }
    }
}

@Composable
private fun AddRegionRow(onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.xl))
            .background(cs.surfaceContainer).clickable(onClick = onClick).padding(16.dp),
    ) {
        Icon(Icons.Rounded.Add, null, tint = cs.primary, modifier = Modifier.size(24.dp))
        Spacer(Modifier.size(12.dp))
        Text("Download a new region", style = MaterialTheme.typography.titleMedium, color = cs.primary)
    }
}
