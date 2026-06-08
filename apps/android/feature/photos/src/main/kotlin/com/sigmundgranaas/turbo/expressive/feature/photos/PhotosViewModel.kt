package com.sigmundgranaas.turbo.expressive.feature.photos

import android.content.Context
import android.net.Uri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.data.PhotoRepository
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Photo
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.util.UUID
import javax.inject.Inject

/** Capture + storage of geotagged photos, optionally attached to a marker. */
@HiltViewModel
class PhotosViewModel @Inject constructor(
    private val repository: PhotoRepository,
    @param:ApplicationContext private val context: Context,
) : ViewModel() {

    fun photosFor(markerId: String): Flow<List<Photo>> = repository.observeForMarker(markerId)

    /** Every geotagged photo, for clustering onto the map. */
    val onMapPhotos: StateFlow<List<Photo>> = repository.observeAll().stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(5_000),
        initialValue = emptyList(),
    )

    /** A fresh app-private file under files/photos for a camera capture target. */
    fun newPhotoFile(): File = File(photoDir(), "${UUID.randomUUID()}.jpg")

    /** Persist a photo already written to [file] (camera capture). */
    fun addCaptured(markerId: String?, position: LatLng, file: File) {
        viewModelScope.launch { insert(markerId, position, file) }
    }

    /** Copy the bytes behind a picked content [uri] into app storage, then persist. */
    fun addFromContent(markerId: String?, position: LatLng, uri: Uri) {
        viewModelScope.launch {
            val file = newPhotoFile()
            val ok = withContext(Dispatchers.IO) {
                runCatching {
                    context.contentResolver.openInputStream(uri)?.use { input ->
                        file.outputStream().use { input.copyTo(it) }
                    } != null
                }.getOrDefault(false)
            }
            if (ok) insert(markerId, position, file)
        }
    }

    fun delete(photo: Photo) {
        viewModelScope.launch {
            withContext(Dispatchers.IO) { runCatching { File(photo.uri).delete() } }
            repository.delete(photo.id)
        }
    }

    private suspend fun insert(markerId: String?, position: LatLng, file: File) {
        repository.add(
            Photo(
                id = "ph-${UUID.randomUUID()}",
                markerId = markerId,
                lat = position.lat,
                lng = position.lng,
                uri = file.absolutePath,
                capturedAtEpochMs = System.currentTimeMillis(),
            ),
        )
    }

    private fun photoDir(): File = File(context.filesDir, "photos").apply { mkdirs() }
}
