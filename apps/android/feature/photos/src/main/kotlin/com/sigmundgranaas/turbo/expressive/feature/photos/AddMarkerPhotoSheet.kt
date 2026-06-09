package com.sigmundgranaas.turbo.expressive.feature.photos

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.PhotoCamera
import androidx.compose.material.icons.rounded.PhotoLibrary
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.core.content.FileProvider
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.ui.components.ListRowItem
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.platform.LocalContext
import java.io.File

/**
 * The photo-source chooser (Camera / Gallery) for a marker — opened from the marker
 * detail's "Add photo" action so the affordance lives in the action options, not as a
 * dedicated button in the body. Captures/imports attach to [markerId] and dismiss.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AddMarkerPhotoSheet(
    markerId: String,
    position: LatLng,
    onDismiss: () -> Unit,
    viewModel: PhotosViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val context = LocalContext.current
    var pendingCapture by remember { mutableStateOf<File?>(null) }

    val galleryLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.PickVisualMedia(),
    ) { uri ->
        if (uri != null) viewModel.addFromContent(markerId, position, uri)
        onDismiss()
    }
    val cameraLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.TakePicture(),
    ) { ok ->
        pendingCapture?.let { if (ok) viewModel.addCaptured(markerId, position, it) }
        onDismiss()
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(),
        shape = RoundedCornerShape(topStart = TurboRadius.xxl, topEnd = TurboRadius.xxl),
        containerColor = cs.surfaceContainerLow,
    ) {
        Column(Modifier.fillMaxWidth().navigationBarsPadding().padding(horizontal = 16.dp, vertical = 4.dp)) {
            ListRowItem(
                icon = Icons.Rounded.PhotoCamera,
                title = "Camera",
                modifier = Modifier.clickable {
                    val file = viewModel.newPhotoFile()
                    pendingCapture = file
                    val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
                    cameraLauncher.launch(uri)
                },
            )
            ListRowItem(
                icon = Icons.Rounded.PhotoLibrary,
                title = "Gallery",
                modifier = Modifier.clickable {
                    galleryLauncher.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
                },
            )
            Spacer(Modifier.height(8.dp))
        }
    }
}
