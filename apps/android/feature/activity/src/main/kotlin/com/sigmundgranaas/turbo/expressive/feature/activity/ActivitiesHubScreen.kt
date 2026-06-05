package com.sigmundgranaas.turbo.expressive.feature.activity

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
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.ui.components.Cookie
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import com.sigmundgranaas.turbo.expressive.ui.theme.icon

/** A saved activity row in the hub. */
data class ActivityEntry(
    val title: String,
    val place: String,
    val kind: ActivityKindId,
    val stat: String,
)

private val sampleActivities = listOf(
    ActivityEntry("Tamokdalen NW", "Tamokdalen · Troms", ActivityKindId.Skiing, "1240 m · L3"),
    ActivityEntry("Skogsfjordvatnet", "Ringvassøya · Troms", ActivityKindId.Fishing, "Trout · open"),
    ActivityEntry("Tønsvika Wall", "Tromsøya · Troms", ActivityKindId.Diving, "−24 m · 12 m vis"),
    ActivityEntry("Tromsdalstind", "Tromsdalen · Troms", ActivityKindId.Hiking, "9.6 km · 1100 m"),
    ActivityEntry("Storsteinen Loop", "Tromsøya · Troms", ActivityKindId.Hiking, "6.1 km · 420 m"),
)

/**
 * Activities hub: a list of the user's saved/planned activities, grouped under a
 * section label, with an FAB that opens the [NewActivitySheet]. Tapping a row
 * opens its [ActivityDetailScreen] (by kind).
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun ActivitiesHubScreen(
    onBack: () -> Unit,
    onOpen: (ActivityKindId) -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    var showPicker by remember { mutableStateOf(false) }
    if (showPicker) {
        NewActivitySheet(
            onPick = { kind -> showPicker = false; onOpen(kind) },
            onDismiss = { showPicker = false },
        )
    }
    Scaffold(
        containerColor = cs.surface,
        topBar = {
            TopAppBar(
                title = { Text("Activities", style = MaterialTheme.typography.headlineSmall) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, "Back") } },
            )
        },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = { showPicker = true },
                icon = { Icon(Icons.Rounded.Add, null) },
                text = { Text("New") },
            )
        },
    ) { pad ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(pad).padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            item {
                Spacer(Modifier.height(4.dp))
                SectionLabel("Saved · ${sampleActivities.size}")
                Spacer(Modifier.height(4.dp))
            }
            items(sampleActivities.size) { i ->
                val a = sampleActivities[i]
                ActivityCard(a) { onOpen(a.kind) }
            }
            item { Spacer(Modifier.height(80.dp)) }
        }
    }
}

@Composable
private fun ActivityCard(entry: ActivityEntry, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.xl))
            .background(cs.surfaceContainerHigh).clickable(onClick = onClick).padding(14.dp),
    ) {
        Cookie(size = 52.dp, fill = cs.primaryContainer) {
            Icon(entry.kind.icon, null, tint = cs.onPrimaryContainer, modifier = Modifier.size(26.dp))
        }
        Spacer(Modifier.size(14.dp))
        Column(Modifier.weight(1f)) {
            Text(entry.title, style = MaterialTheme.typography.titleMedium, color = cs.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("${entry.place} · ${entry.stat}", style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Box(Modifier.size(28.dp), contentAlignment = Alignment.Center) {
            Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null, tint = cs.onSurfaceVariant)
        }
    }
}
