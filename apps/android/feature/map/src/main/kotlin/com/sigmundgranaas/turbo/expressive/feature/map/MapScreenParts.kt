package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.feature.map.route.RouteUiState
import com.sigmundgranaas.turbo.expressive.feature.map.route.RouteViewModel

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CloudOff
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker

/**
 * Self-contained pieces of the map host pulled out of [MapScreen]: a couple of
 * pure helpers and the small status pills shown under the search bar. They carry
 * no host state, so they live here to keep `MapScreen` focused on composition.
 */

/** How close a saved marker must sit to the planned route to count as a checkpoint (D3). */
private const val CHECKPOINT_NEAR_M = 40.0

/**
 * Saved markers within [CHECKPOINT_NEAR_M] of the solved route, as (position, name) checkpoints
 * (D3). [RouteViewModel.follow] merges these with the route stops and orders both by arc-length.
 */
internal fun nearbyCheckpoints(state: RouteUiState, markers: List<Marker>): List<Pair<LatLng, String>> {
    val geometry = (state as? RouteUiState.Done)?.plan?.geometry ?: return emptyList()
    return markers
        .filter { GeoMetrics.distanceToPath(geometry, it.position) <= CHECKPOINT_NEAR_M }
        .map { it.position to it.name }
}

/** Export a single marker as a .geojson file and fire a share chooser. */
internal fun shareMarkerGeoJson(context: android.content.Context, marker: Marker) {
    val dir = java.io.File(context.cacheDir, "markers").apply { mkdirs() }
    val file = java.io.File(dir, com.sigmundgranaas.turbo.expressive.feature.markers.MarkerGeoJson.fileName(marker.name))
    file.writeText(com.sigmundgranaas.turbo.expressive.feature.markers.MarkerGeoJson.encode(listOf(marker)))
    val uri = androidx.core.content.FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
    val send = android.content.Intent(android.content.Intent.ACTION_SEND).apply {
        type = "application/geo+json"
        putExtra(android.content.Intent.EXTRA_STREAM, uri)
        clipData = android.content.ClipData.newRawUri(marker.name, uri)
        addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
    context.startActivity(android.content.Intent.createChooser(send, "Share marker").addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK))
}

/** Small pill shown under the search bar while waiting for the first GPS fix. */
@Composable
internal fun LocatingChip(modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(50),
        color = cs.surfaceContainerHigh,
        shadowElevation = 3.dp,
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
        ) {
            CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp, color = cs.primary)
            Spacer(Modifier.width(10.dp))
            Text(
                stringResource(R.string.location_finding),
                style = MaterialTheme.typography.labelLarge,
                color = cs.onSurface,
            )
        }
    }
}

/** "You're offline" pill under the search bar; says so louder when the camera is
 *  also outside every downloaded region (i.e. the basemap will be blank). */
@Composable
internal fun OfflineChip(outsideCoverage: Boolean, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(50),
        color = if (outsideCoverage) cs.errorContainer else cs.surfaceContainerHigh,
        shadowElevation = 3.dp,
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
        ) {
            Icon(
                Icons.Rounded.CloudOff,
                null,
                tint = if (outsideCoverage) cs.onErrorContainer else cs.primary,
                modifier = Modifier.size(16.dp),
            )
            Spacer(Modifier.width(10.dp))
            Text(
                stringResource(if (outsideCoverage) R.string.offline_chip_uncovered else R.string.offline_chip),
                style = MaterialTheme.typography.labelLarge,
                color = if (outsideCoverage) cs.onErrorContainer else cs.onSurface,
            )
        }
    }
}
