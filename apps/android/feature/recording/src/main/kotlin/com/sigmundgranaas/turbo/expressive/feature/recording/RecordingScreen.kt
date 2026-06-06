package com.sigmundgranaas.turbo.expressive.feature.recording

import android.Manifest
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.core.geo.Units
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.SampleData
import com.sigmundgranaas.turbo.expressive.ui.map.MapController
import com.sigmundgranaas.turbo.expressive.ui.theme.LocalMetricUnits
import com.sigmundgranaas.turbo.expressive.ui.map.TurboMap

@Composable
fun RecordingScreen(
    onStop: () -> Unit,
    viewModel: RecordingViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val ui by viewModel.state.collectAsStateWithLifecycle()
    val metric = LocalMetricUnits.current
    var controller by remember { mutableStateOf<MapController?>(null) }
    var showSave by remember { mutableStateOf(false) }

    val permissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted -> viewModel.onPermissionResult(granted) }
    val notificationPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { /* best-effort: the foreground notification only shows if granted */ }

    // On entry: ensure the ongoing notification can show (Android 13+), then start
    // if permitted, else request the location permission.
    LaunchedEffect(Unit) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        if (ui.hasPermission) viewModel.start() else permissionLauncher.launch(Manifest.permission.ACCESS_FINE_LOCATION)
    }
    // Keep the camera on the latest fix.
    LaunchedEffect(ui.userLocation) {
        ui.userLocation?.let { controller?.flyTo(it, 15.0) }
    }

    Box(Modifier.fillMaxSize()) {
        TurboMap(
            base = BaseLayer.Norgeskart,
            initialCamera = ui.userLocation ?: SampleData.initialCamera,
            initialZoom = 14.0,
            route = ui.points.takeIf { it.size > 1 },
            routeColor = cs.primary,
            userLocation = ui.userLocation,
            onMapReady = { controller = it },
            modifier = Modifier.fillMaxSize(),
        )

        StatusChip(
            recording = ui.recording,
            paused = ui.paused,
            elapsed = formatElapsed(ui.elapsedSec),
            modifier = Modifier.align(Alignment.TopCenter).windowInsetsPadding(WindowInsets.statusBars).padding(top = 16.dp),
        )

        Column(Modifier.align(Alignment.BottomCenter).fillMaxWidth()) {
            Surface(
                shape = RoundedCornerShape(28.dp),
                color = cs.surfaceContainerHigh,
                shadowElevation = 3.dp,
                modifier = Modifier.padding(horizontal = 16.dp).fillMaxWidth(),
            ) {
                Row(Modifier.padding(horizontal = 20.dp, vertical = 18.dp)) {
                    Stat(Units.distance(ui.distanceM, metric), "", "Distance", Modifier.weight(1f))
                    Stat(formatElapsed(ui.elapsedSec), "", "Time", Modifier.weight(1f))
                    Stat(Units.pace(ui.distanceM, ui.elapsedSec, metric), "", "Pace", Modifier.weight(1f))
                }
            }
            Spacer(Modifier.height(12.dp))

            Surface(color = cs.surfaceContainer, modifier = Modifier.fillMaxWidth()) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.End,
                    modifier = Modifier.fillMaxWidth()
                        .windowInsetsPadding(WindowInsets.navigationBars)
                        .padding(horizontal = 16.dp, vertical = 14.dp),
                ) {
                    FloatingActionButton(
                        onClick = { viewModel.togglePause() },
                        containerColor = cs.secondaryContainer,
                        contentColor = cs.onSecondaryContainer,
                        modifier = Modifier.size(56.dp),
                    ) { Icon(if (ui.paused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause, if (ui.paused) "Resume" else "Pause") }
                    Spacer(Modifier.width(12.dp))
                    FloatingActionButton(
                        onClick = { viewModel.stop(); showSave = true },
                        containerColor = cs.primary,
                        contentColor = cs.onPrimary,
                        modifier = Modifier.size(56.dp),
                    ) { Icon(Icons.Rounded.Stop, "Stop") }
                }
            }
        }
    }

    if (showSave) {
        SaveTrackDialog(
            defaultName = "Track ${Units.distance(ui.distanceM, metric)}",
            canSave = ui.points.size > 1,
            onSave = { name -> viewModel.save(name) { onStop() } },
            onDiscard = { viewModel.discard { onStop() } },
            onDismiss = { showSave = false },
        )
    }
}

@Composable
private fun StatusChip(recording: Boolean, paused: Boolean, elapsed: String, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    val live = recording && !paused
    Surface(shape = CircleShape, color = cs.surfaceContainerHigh, shadowElevation = 3.dp, modifier = modifier) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 20.dp, vertical = 11.dp)) {
            Box(Modifier.size(11.dp).clip(CircleShape).background(if (live) Color(0xFFE0432B) else cs.onSurfaceVariant))
            Spacer(Modifier.width(10.dp))
            Text(
                text = (if (live) "Recording" else "Paused") + " · " + elapsed,
                style = MaterialTheme.typography.titleSmall,
                color = cs.onSurface,
            )
        }
    }
}

@Composable
private fun SaveTrackDialog(
    defaultName: String,
    canSave: Boolean,
    onSave: (String) -> Unit,
    onDiscard: () -> Unit,
    onDismiss: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    var name by remember { mutableStateOf(defaultName) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (canSave) "Save track?" else "Nothing recorded") },
        text = {
            if (canSave) {
                Surface(shape = RoundedCornerShape(12.dp), color = cs.surfaceContainerHigh) {
                    BasicTextField(
                        value = name,
                        onValueChange = { name = it },
                        singleLine = true,
                        textStyle = MaterialTheme.typography.bodyLarge.copy(color = cs.onSurface),
                        cursorBrush = SolidColor(cs.primary),
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 14.dp),
                    )
                }
            } else {
                Text("No track points were captured.", color = cs.onSurfaceVariant)
            }
        },
        confirmButton = {
            if (canSave) Button(onClick = { onSave(name) }) { Text("Save") } else Button(onClick = onDiscard) { Text("Done") }
        },
        dismissButton = { if (canSave) TextButton(onClick = onDiscard) { Text("Discard") } },
    )
}

@Composable
private fun Stat(value: String, unit: String, label: String, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Column(modifier) {
        Row(verticalAlignment = Alignment.Bottom) {
            Text(value, style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.W800), color = cs.onSurface)
            if (unit.isNotEmpty()) {
                Spacer(Modifier.width(3.dp))
                Text(unit, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant, modifier = Modifier.padding(bottom = 3.dp))
            }
        }
        Text(label, style = MaterialTheme.typography.labelMedium, color = cs.onSurfaceVariant)
    }
}

private fun formatElapsed(seconds: Int): String {
    val h = seconds / 3600
    val m = (seconds % 3600) / 60
    val s = seconds % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%02d:%02d".format(m, s)
}
