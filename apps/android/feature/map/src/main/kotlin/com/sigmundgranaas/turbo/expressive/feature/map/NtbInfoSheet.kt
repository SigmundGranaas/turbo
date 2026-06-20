package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Cabin
import androidx.compose.material.icons.rounded.Hiking
import androidx.compose.material.icons.rounded.OpenInNew
import androidx.compose.material.icons.rounded.Place
import androidx.compose.material.icons.rounded.Route
import androidx.compose.material.icons.rounded.Straighten
import androidx.compose.material3.AssistChip
import androidx.compose.material3.AssistChipDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.core.geo.Units
import com.sigmundgranaas.turbo.expressive.domain.NtbPoi
import com.sigmundgranaas.turbo.expressive.domain.NtbPoiType
import com.sigmundgranaas.turbo.expressive.domain.NtbRoute
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

/**
 * Bottom sheet for a tapped Nasjonal Turbase (ut.no / DNT) POI — cabin, trip or
 * place. Shows the title, summary and (for a trip) distance/grade chips, plus a
 * deep link back to ut.no. The trip's route is revealed on the map behind it.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun NtbInfoSheet(
    poi: NtbPoi,
    route: NtbRoute?,
    metric: Boolean,
    onOpenUt: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        shape = RoundedCornerShape(topStart = TurboRadius.xxl, topEnd = TurboRadius.xxl),
        containerColor = cs.surfaceContainerLow,
    ) {
        Column(
            Modifier
                .navigationBarsPadding()
                .padding(start = 24.dp, end = 24.dp, bottom = 28.dp),
        ) {
            // Header: type icon + title.
            androidx.compose.foundation.layout.Row(
                verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
            ) {
                Icon(poi.type.icon(), null, tint = cs.primary, modifier = Modifier.size(28.dp))
                androidx.compose.foundation.layout.Spacer(Modifier.size(12.dp))
                Text(
                    poi.title,
                    style = MaterialTheme.typography.headlineSmall,
                    color = cs.onSurface,
                )
            }

            poi.summary?.takeIf { it.isNotBlank() }?.let {
                androidx.compose.foundation.layout.Spacer(Modifier.height(12.dp))
                Text(it, style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
            }

            // Trip metadata: distance + grade chips.
            if (route != null && (route.distanceMeters != null || route.grade != null)) {
                androidx.compose.foundation.layout.Spacer(Modifier.height(14.dp))
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    route.distanceMeters?.let { d ->
                        InfoChip(Icons.Rounded.Straighten, Units.distance(d, metric))
                    }
                    route.grade?.takeIf { it.isNotBlank() }?.let { g ->
                        InfoChip(Icons.Rounded.Route, g)
                    }
                }
            }

            // Deep link back to ut.no.
            (route?.utUrl ?: poi.utUrl)?.takeIf { it.isNotBlank() }?.let { url ->
                androidx.compose.foundation.layout.Spacer(Modifier.height(20.dp))
                FilledTonalButton(onClick = { onOpenUt(url) }, modifier = Modifier.fillMaxWidth()) {
                    Icon(Icons.Rounded.OpenInNew, null, modifier = Modifier.size(20.dp))
                    androidx.compose.foundation.layout.Spacer(Modifier.size(8.dp))
                    Text(stringResource(R.string.ntb_open_ut))
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun InfoChip(icon: ImageVector, label: String) {
    AssistChip(
        onClick = {},
        enabled = false,
        label = { Text(label) },
        leadingIcon = { Icon(icon, null, modifier = Modifier.size(18.dp)) },
        colors = AssistChipDefaults.assistChipColors(
            disabledLabelColor = MaterialTheme.colorScheme.onSurface,
            disabledLeadingIconContentColor = MaterialTheme.colorScheme.primary,
        ),
    )
}

private fun NtbPoiType.icon(): ImageVector = when (this) {
    NtbPoiType.Cabin -> Icons.Rounded.Cabin
    NtbPoiType.Trip -> Icons.Rounded.Hiking
    NtbPoiType.Place -> Icons.Rounded.Place
}
