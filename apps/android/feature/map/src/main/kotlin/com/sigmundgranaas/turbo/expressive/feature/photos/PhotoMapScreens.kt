package com.sigmundgranaas.turbo.expressive.feature.photos

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items as rowItems
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.PhotoCamera
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.sigmundgranaas.turbo.expressive.domain.Photo
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter

private val DAY = DateTimeFormatter.ofPattern("d MMM")
private val DAY_YEAR = DateTimeFormatter.ofPattern("d MMM yyyy")
private val FULL = DateTimeFormatter.ofPattern("d MMMM yyyy")
private val TIME = DateTimeFormatter.ofPattern("HH:mm")

private fun Long.localDate() = Instant.ofEpochMilli(this).atZone(ZoneId.systemDefault()).toLocalDate()
private fun Long.localTime() = Instant.ofEpochMilli(this).atZone(ZoneId.systemDefault())

/** "12 Mar – 14 Mar 2025" / "12 Mar 2025" for a set of photos. */
internal fun photoDateRange(photos: List<Photo>): String {
    if (photos.isEmpty()) return ""
    val dates = photos.map { it.capturedAtEpochMs.localDate() }
    val min = dates.min(); val max = dates.max()
    return if (min == max) min.format(DAY_YEAR) else "${min.format(DAY)} – ${max.format(DAY_YEAR)}"
}

/** A tapped photo stack: place/date header + a thumbnail grid that opens the viewer. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun PhotoClusterSheet(cluster: PhotoCluster, onOpen: (Int) -> Unit, onDismiss: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    val photos = cluster.ordered
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = cs.surfaceContainerLow,
    ) {
        Column(Modifier.padding(start = 18.dp, end = 18.dp, bottom = 24.dp).testTag("photoClusterSheet")) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 16.dp)) {
                Box(
                    Modifier.size(52.dp).clip(CircleShape).background(cs.primaryContainer),
                    contentAlignment = Alignment.Center,
                ) { Icon(Icons.Rounded.PhotoCamera, null, tint = cs.onPrimaryContainer, modifier = Modifier.size(26.dp)) }
                Spacer(Modifier.width(14.dp))
                Column(Modifier.weight(1f)) {
                    Text(
                        "${cluster.count} ${if (cluster.count == 1) "photo" else "photos"}",
                        style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.W800),
                        color = cs.onSurface,
                    )
                    Text(photoDateRange(photos), style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
                }
            }
            LazyVerticalGrid(
                columns = GridCells.Fixed(3),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
                modifier = Modifier.fillMaxWidth().heightInClamp(),
            ) {
                items(photos.size) { i ->
                    val bmp = rememberPhotoBitmap(photos[i].uri, maxPx = 320)
                    Box(
                        Modifier.aspectRatio(1f).clip(RoundedCornerShape(if (i == 0) 18.dp else 12.dp))
                            .background(cs.surfaceContainerHigh).clickable { onOpen(i) },
                        contentAlignment = Alignment.Center,
                    ) {
                        if (bmp != null) Image(bmp, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
                    }
                }
            }
        }
    }
}

// Keep the grid from growing unbounded inside the sheet's scroll column.
private fun Modifier.heightInClamp(): Modifier = this.height(460.dp)

/** Immersive full-screen viewer: swipeable pager, date/time bar, filmstrip, delete. */
@Composable
internal fun PhotoViewer(photos: List<Photo>, startIndex: Int, onClose: () -> Unit, onDelete: (Photo) -> Unit) {
    if (photos.isEmpty()) return
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        val pager = rememberPagerState(initialPage = startIndex.coerceIn(0, photos.lastIndex)) { photos.size }
        Box(Modifier.fillMaxSize().background(Color(0xFF0C0A09)).testTag("photoViewer")) {
            HorizontalPager(state = pager, modifier = Modifier.fillMaxSize()) { page ->
                val full = rememberPhotoBitmap(photos[page].uri, maxPx = 1600)
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    if (full != null) Image(full, "Photo", Modifier.fillMaxSize(), contentScale = ContentScale.Fit)
                }
            }

            // top bar — clear of the status bar (full-screen dialog draws edge-to-edge).
            val current = photos[pager.currentPage]
            Row(
                Modifier.fillMaxWidth().statusBarsPadding().padding(top = 14.dp, start = 8.dp, end = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = onClose) { Icon(Icons.Rounded.Close, "Close", tint = Color.White) }
                Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(current.capturedAtEpochMs.localDate().format(FULL), style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.W700), color = Color.White)
                    Text(current.capturedAtEpochMs.localTime().format(TIME), style = MaterialTheme.typography.labelMedium, color = Color.White.copy(alpha = 0.7f))
                }
                IconButton(onClick = { onDelete(current) }) { Icon(Icons.Rounded.DeleteOutline, "Delete", tint = Color.White) }
            }

            // filmstrip — lifted above the gesture-nav bar.
            LazyRow(
                Modifier.align(Alignment.BottomCenter).fillMaxWidth().navigationBarsPadding().padding(bottom = 20.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally),
                contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 14.dp),
            ) {
                rowItems(photos) { p ->
                    val active = p.id == current.id
                    val thumb = rememberPhotoBitmap(p.uri, maxPx = 120)
                    Box(
                        Modifier.size(if (active) 46.dp else 38.dp).clip(RoundedCornerShape(11.dp))
                            .background(Color.White.copy(alpha = 0.12f)),
                    ) { if (thumb != null) Image(thumb, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop) }
                }
            }
        }
    }
}
