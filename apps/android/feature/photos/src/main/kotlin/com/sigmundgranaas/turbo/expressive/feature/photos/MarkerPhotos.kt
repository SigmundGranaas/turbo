package com.sigmundgranaas.turbo.expressive.feature.photos

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AddAPhoto
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.PhotoCamera
import androidx.compose.material.icons.rounded.PhotoLibrary
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.core.content.FileProvider
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Photo
import com.sigmundgranaas.turbo.expressive.ui.components.pressScale
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import java.io.File

/** Horizontal strip of a marker's photos + an add control (camera / gallery). */
@Composable
fun MarkerPhotos(
    markerId: String,
    position: LatLng,
    modifier: Modifier = Modifier,
    viewModel: PhotosViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val context = LocalContext.current
    val photos by remember(markerId) { viewModel.photosFor(markerId) }
        .collectAsStateWithLifecycle(emptyList())
    var viewing by remember { mutableStateOf<Photo?>(null) }
    var addMenu by remember { mutableStateOf(false) }
    var pendingCapture by remember { mutableStateOf<File?>(null) }

    val galleryLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.PickVisualMedia(),
    ) { uri -> if (uri != null) viewModel.addFromContent(markerId, position, uri) }

    val cameraLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.TakePicture(),
    ) { success -> pendingCapture?.let { if (success) viewModel.addCaptured(markerId, position, it) } }

    Column(modifier.fillMaxWidth()) {
        // A clearly-labelled "Add photo" affordance (camera / gallery) rather than a bare
        // 72dp icon tile that reads as a mystery thumbnail. The strip below is photos only.
        Box {
            val addSource = remember { MutableInteractionSource() }
            FilledTonalButton(
                onClick = { addMenu = true },
                interactionSource = addSource,
                modifier = Modifier.pressScale(addSource),
            ) {
                Icon(Icons.Rounded.AddAPhoto, null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.size(8.dp))
                Text("Add photo")
            }
            DropdownMenu(expanded = addMenu, onDismissRequest = { addMenu = false }) {
                DropdownMenuItem(
                    text = { Text("Camera") },
                    leadingIcon = { Icon(Icons.Rounded.PhotoCamera, null) },
                    onClick = {
                        addMenu = false
                        val file = viewModel.newPhotoFile()
                        pendingCapture = file
                        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
                        cameraLauncher.launch(uri)
                    },
                )
                DropdownMenuItem(
                    text = { Text("Gallery") },
                    leadingIcon = { Icon(Icons.Rounded.PhotoLibrary, null) },
                    onClick = {
                        addMenu = false
                        galleryLauncher.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
                    },
                )
            }
        }
        if (photos.isNotEmpty()) {
            Spacer(Modifier.size(12.dp))
            LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
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
