package com.sigmundgranaas.turbo.expressive.ui.components

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Flag
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.MapEngine
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.ui.theme.icon
import kotlin.math.roundToInt

/** A geotagged photo (or photo cluster) rendered on the map as a framed thumbnail. */
data class PhotoPin(val id: String, val lat: Double, val lng: Double, val count: Int, val coverPath: String?)

/**
 * The renderer-agnostic Compose overlay layer: markers, editable route waypoints, and
 * photo pins, **projected through the [MapEngine] seam** ([MapEngine.toScreen] /
 * [MapEngine.fromScreen]) rather than any one renderer's projection. Both the MapLibre
 * host (`TurboMap`) and the wgpu host (`TurbomapMapView`) draw their pins with this, so
 * the on-map UI is pixel-identical regardless of which engine is behind it.
 *
 * [cameraTick] is bumped by the host on every camera change so the offsets reproject in
 * lockstep with the map (markers don't drift during pan/zoom). Place this in the same
 * box as the map surface, filling it.
 */
@Composable
fun MapOverlay(
    engine: MapEngine,
    cameraTick: Int,
    modifier: Modifier = Modifier,
    markers: List<Marker> = emptyList(),
    selectedMarkerId: String? = null,
    markerFallbackColor: Color = Color(0xFF8F4C38),
    onMarkerClick: (Marker) -> Unit = {},
    photoPins: List<PhotoPin> = emptyList(),
    onPhotoPinClick: (PhotoPin) -> Unit = {},
    waypoints: List<LatLng> = emptyList(),
    selectedWaypoint: Int? = null,
    onWaypointTap: (Int) -> Unit = {},
    onWaypointLongPress: (Int) -> Unit = {},
    onWaypointMoved: (Int, LatLng) -> Unit = { _, _ -> },
) {
    val density = LocalDensity.current
    Box(modifier.fillMaxSize()) {
        photoPins.forEach { pin ->
            val boxPx = with(density) { 56.dp.toPx() }
            Box(
                Modifier
                    .offset {
                        @Suppress("UNUSED_EXPRESSION") cameraTick
                        val (x, y) = engine.toScreen(LatLng(pin.lat, pin.lng))
                        IntOffset((x - boxPx / 2f).roundToInt(), (y - boxPx / 2f).roundToInt())
                    }
                    .testTag("photoPin"),
            ) {
                PhotoPinView(pin = pin, onClick = { onPhotoPinClick(pin) })
            }
        }
        markers.forEach { m ->
            val selected = m.id == selectedMarkerId
            val boxPx = with(density) { (if (selected) 42.dp else 33.dp).toPx() }
            MarkerPin(
                icon = m.kind.icon,
                selected = selected,
                color = m.colorArgb?.let { Color(it) } ?: markerFallbackColor,
                modifier = Modifier
                    .offset {
                        @Suppress("UNUSED_EXPRESSION") cameraTick
                        val (x, y) = engine.toScreen(m.position)
                        IntOffset((x - boxPx / 2f).roundToInt(), (y - boxPx).roundToInt())
                    }
                    .clickable { onMarkerClick(m) },
            )
        }
        // Editable route waypoints — drawn last so they sit above markers.
        waypoints.forEachIndexed { index, wp ->
            WaypointMarkerView(
                index = index,
                last = waypoints.lastIndex,
                selected = index == selectedWaypoint,
                cameraTick = cameraTick,
                project = { engine.toScreen(wp).let { Offset(it.first, it.second) } },
                toLatLng = { o -> engine.fromScreen(o.x, o.y) },
                onTap = { onWaypointTap(index) },
                onLongPress = { onWaypointLongPress(index) },
                onMoved = { onWaypointMoved(index, it) },
            )
        }
    }
}

private val WpStart = Color(0xFF2E7D32)
private val WpEnd = Color(0xFFC0392B)

/**
 * An on-map route waypoint: an A/B/C… letter badge (flag for the destination), start
 * green / end red / via primary, with a selected ring. Tap selects, long-press removes,
 * dragging moves it ([onMoved] fires once on drop with the new position).
 */
@Composable
internal fun WaypointMarkerView(
    index: Int,
    last: Int,
    selected: Boolean,
    cameraTick: Int,
    project: () -> Offset,
    toLatLng: (Offset) -> LatLng,
    onTap: () -> Unit,
    onLongPress: () -> Unit,
    onMoved: (LatLng) -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    val density = LocalDensity.current
    val sizeDp = if (selected) 40.dp else 32.dp
    val boxPx = with(density) { sizeDp.toPx() }
    var drag by remember { mutableStateOf(Offset.Zero) }
    val color = when (index) {
        0 -> WpStart
        last -> WpEnd
        else -> cs.primary
    }
    Box(
        Modifier
            .offset {
                @Suppress("UNUSED_EXPRESSION") cameraTick
                val p = project()
                IntOffset((p.x - boxPx / 2f + drag.x).roundToInt(), (p.y - boxPx / 2f + drag.y).roundToInt())
            }
            .size(sizeDp)
            .testTag("waypoint_$index")
            .pointerInput(index) {
                detectDragGestures(
                    onDrag = { change, amount -> change.consume(); drag += amount },
                    onDragEnd = { onMoved(toLatLng(project() + drag)); drag = Offset.Zero },
                    onDragCancel = { drag = Offset.Zero },
                )
            }
            .pointerInput(index) {
                detectTapGestures(onTap = { onTap() }, onLongPress = { onLongPress() })
            },
        contentAlignment = Alignment.Center,
    ) {
        if (selected) {
            Box(Modifier.size(sizeDp + 18.dp).clip(CircleShape).background(color.copy(alpha = 0.20f)))
        }
        Box(
            Modifier.size(sizeDp).clip(CircleShape).background(color).border(3.dp, cs.surface, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            if (index == last) {
                Icon(Icons.Rounded.Flag, null, tint = Color.White, modifier = Modifier.size(if (selected) 20.dp else 16.dp))
            } else {
                Text(
                    ('A' + index).toString(),
                    style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.W800),
                    color = Color.White,
                )
            }
        }
    }
}

/** White-framed rounded thumbnail with a count badge for clusters. */
@Composable
private fun PhotoPinView(pin: PhotoPin, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Box(Modifier.size(56.dp)) {
        Surface(
            onClick = onClick,
            shape = RoundedCornerShape(16.dp),
            color = cs.surface,
            shadowElevation = 6.dp,
            modifier = Modifier.size(56.dp),
        ) {
            Box(Modifier.fillMaxSize().padding(3.dp).clip(RoundedCornerShape(13.dp)).background(cs.surfaceContainerHighest)) {
                val bmp = pin.coverPath?.let { rememberThumb(it) }
                if (bmp != null) Image(bmp, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
            }
        }
        if (pin.count > 1) {
            Surface(
                shape = CircleShape,
                color = cs.primary,
                shadowElevation = 2.dp,
                modifier = Modifier.align(Alignment.TopEnd).offset(x = 6.dp, y = (-6).dp),
            ) {
                Box(Modifier.size(22.dp), contentAlignment = Alignment.Center) {
                    Text(
                        "${pin.count}",
                        style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.W700),
                        color = cs.onPrimary,
                    )
                }
            }
        }
    }
}

/** Decodes [path] off the main thread, downsampled for an on-map thumbnail. */
@Composable
private fun rememberThumb(path: String): ImageBitmap? {
    val state by produceState<ImageBitmap?>(initialValue = null, path) {
        value = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
            runCatching {
                val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeFile(path, bounds)
                var sample = 1
                while (bounds.outWidth / sample > 160 || bounds.outHeight / sample > 160) sample *= 2
                BitmapFactory.decodeFile(path, BitmapFactory.Options().apply { inSampleSize = sample })?.asImageBitmap()
            }.getOrNull()
        }
    }
    return state
}
