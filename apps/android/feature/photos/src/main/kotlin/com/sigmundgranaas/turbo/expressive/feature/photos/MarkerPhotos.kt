package com.sigmundgranaas.turbo.expressive.feature.photos

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.domain.Photo
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

/**
 * Horizontal strip of a marker's photos (tap to view full-screen / delete). Adding is
 * driven from the marker detail's "Add photo" action — see [AddMarkerPhotoSheet] — so
 * this is display-only and renders nothing when the marker has no photos.
 */
@Composable
fun MarkerPhotos(
    markerId: String,
    modifier: Modifier = Modifier,
    viewModel: PhotosViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val photos by remember(markerId) { viewModel.photosFor(markerId) }
        .collectAsStateWithLifecycle(emptyList())
    var viewing by remember { mutableStateOf<Photo?>(null) }

    if (photos.isNotEmpty()) {
        LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = modifier.fillMaxWidth()) {
            items(photos.size) { i ->
                val photo = photos[i]
                val bmp = rememberPhotoBitmap(photo.uri, maxPx = 256)
                Box(
                    Modifier.size(72.dp).clip(RoundedCornerShape(TurboRadius.m)).background(cs.surfaceContainerHigh)
                        .clickable { viewing = photo },
                    contentAlignment = Alignment.Center,
                ) {
                    if (bmp != null) Image(bmp, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
                }
            }
        }
    }

    viewing?.let { photo ->
        Dialog(onDismissRequest = { viewing = null }) {
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.TopEnd) {
                val full = rememberPhotoBitmap(photo.uri, maxPx = 1280)
                if (full != null) {
                    Image(full, "Photo", Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.l)), contentScale = ContentScale.Fit)
                }
                IconButton(
                    onClick = { viewModel.delete(photo); viewing = null },
                    modifier = Modifier.padding(6.dp).clip(RoundedCornerShape(50)).background(Color.Black.copy(alpha = 0.4f)),
                ) { Icon(Icons.Rounded.DeleteOutline, "Delete photo", tint = Color.White) }
            }
        }
    }
}

// rememberPhotoBitmap moved to PhotoImage.kt (shared with the on-map photo layer).
