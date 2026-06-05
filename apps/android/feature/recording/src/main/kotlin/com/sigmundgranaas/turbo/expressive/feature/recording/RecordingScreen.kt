package com.sigmundgranaas.turbo.expressive.feature.recording

import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AddLocationAlt
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PhotoCamera
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.SampleData
import com.sigmundgranaas.turbo.expressive.ui.map.TurboMap

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun RecordingScreen(
    onStop: () -> Unit,
    viewModel: RecordingViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val ui by viewModel.state.collectAsStateWithLifecycle()
    val paused = ui.paused

    Box(Modifier.fillMaxSize()) {
        TurboMap(
            base = BaseLayer.Norgeskart,
            initialCamera = SampleData.storsteinenLoop[3],
            initialZoom = 12.0,
            route = SampleData.storsteinenLoop,
            routeColor = cs.primary,
            modifier = Modifier.fillMaxSize(),
        )

        // Status chip
        Surface(
            shape = CircleShape,
            color = cs.surfaceContainerHigh,
            shadowElevation = 3.dp,
            modifier = Modifier.align(Alignment.TopCenter).windowInsetsPadding(WindowInsets.statusBars).padding(top = 16.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 20.dp, vertical = 11.dp)) {
                Box(Modifier.size(11.dp).clip(CircleShape).background(if (paused) cs.onSurfaceVariant else androidx.compose.ui.graphics.Color(0xFFE0432B)))
                Spacer(Modifier.width(10.dp))
                Text(if (paused) "Paused · 00:42:18" else "Recording · 00:42:18", style = MaterialTheme.typography.titleSmall, color = cs.onSurface)
            }
        }

        Column(Modifier.align(Alignment.BottomCenter).fillMaxWidth()) {
            // Live stats + wavy progress
            Surface(
                shape = RoundedCornerShape(28.dp),
                color = cs.surfaceContainerHigh,
                shadowElevation = 3.dp,
                modifier = Modifier.padding(horizontal = 16.dp).fillMaxWidth(),
            ) {
                Column(Modifier.padding(horizontal = 20.dp, vertical = 16.dp)) {
                    Row(Modifier.fillMaxWidth()) {
                        Stat("6.2", "km", "Distance", Modifier.weight(1f))
                        Stat("480", "m", "Ascent", Modifier.weight(1f))
                        Stat("5:48", "/km", "Pace", Modifier.weight(1f))
                    }
                    Spacer(Modifier.height(14.dp))
                    LinearWavyProgressIndicator(
                        progress = { if (paused) 0.001f else 0.62f },
                        modifier = Modifier.fillMaxWidth().height(10.dp),
                        color = if (paused) cs.onSurfaceVariant else cs.primary,
                    )
                }
            }
            Spacer(Modifier.height(12.dp))

            // Expressive bottom app bar with docked FABs
            Surface(color = cs.surfaceContainer, modifier = Modifier.fillMaxWidth()) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth()
                        .windowInsetsPadding(WindowInsets.navigationBars)
                        .padding(start = 8.dp, end = 12.dp, top = 12.dp, bottom = 12.dp),
                ) {
                    IconButton(onClick = {}) { Icon(Icons.Rounded.Layers, "Layers", tint = cs.onSurfaceVariant) }
                    IconButton(onClick = {}) { Icon(Icons.Rounded.AddLocationAlt, "Mark point", tint = cs.onSurfaceVariant) }
                    IconButton(onClick = {}) { Icon(Icons.Rounded.PhotoCamera, "Photo", tint = cs.onSurfaceVariant) }
                    Spacer(Modifier.weight(1f))
                    FloatingActionButton(
                        onClick = { viewModel.togglePause() },
                        containerColor = cs.secondaryContainer,
                        contentColor = cs.onSecondaryContainer,
                        modifier = Modifier.size(56.dp),
                    ) { Icon(if (paused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause, if (paused) "Resume" else "Pause") }
                    Spacer(Modifier.width(10.dp))
                    FloatingActionButton(
                        onClick = onStop,
                        containerColor = cs.primary,
                        contentColor = cs.onPrimary,
                        modifier = Modifier.size(56.dp),
                    ) { Icon(Icons.Rounded.Stop, "Stop") }
                }
            }
        }
    }
}

@Composable
private fun Stat(value: String, unit: String, label: String, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Column(modifier) {
        Row(verticalAlignment = Alignment.Bottom) {
            Text(value, style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.W800), color = cs.onSurface)
            Spacer(Modifier.width(3.dp))
            Text(unit, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant, modifier = Modifier.padding(bottom = 3.dp))
        }
        Text(label, style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant)
    }
}
