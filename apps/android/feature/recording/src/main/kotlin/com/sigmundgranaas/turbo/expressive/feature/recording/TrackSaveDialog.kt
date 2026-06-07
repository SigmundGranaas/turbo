package com.sigmundgranaas.turbo.expressive.feature.recording

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import com.sigmundgranaas.turbo.expressive.ui.theme.labelRes

/**
 * Name + activity-kind picker shown when a recording stops. Lives outside any one
 * screen so both the (now retired) standalone recorder and the home-map recording
 * mode can finish a track through the exact same UX.
 */
@Composable
fun TrackSaveDialog(
    defaultName: String,
    canSave: Boolean,
    onSave: (String, ActivityKindId) -> Unit,
    onDiscard: () -> Unit,
    onDismiss: () -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    var name by remember { mutableStateOf(defaultName) }
    var kind by remember { mutableStateOf(ActivityKindId.Hiking) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(if (canSave) R.string.rec_save_title else R.string.rec_nothing_title)) },
        text = {
            if (canSave) {
                Column {
                    Surface(shape = RoundedCornerShape(TurboRadius.s), color = cs.surfaceContainerHigh) {
                        BasicTextField(
                            value = name,
                            onValueChange = { name = it },
                            singleLine = true,
                            textStyle = MaterialTheme.typography.bodyLarge.copy(color = cs.onSurface),
                            cursorBrush = SolidColor(cs.primary),
                            modifier = Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 14.dp).testTag("trackName"),
                        )
                    }
                    Spacer(Modifier.padding(top = 8.dp))
                    Row(
                        Modifier.horizontalScroll(rememberScrollState()),
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        ActivityKindId.entries.forEach { k ->
                            FilterChip(
                                selected = k == kind,
                                onClick = { kind = k },
                                label = { Text(stringResource(k.labelRes)) },
                            )
                        }
                    }
                }
            } else {
                Text(stringResource(R.string.rec_no_points), color = cs.onSurfaceVariant)
            }
        },
        confirmButton = {
            if (canSave) {
                Button(onClick = { onSave(name, kind) }, modifier = Modifier.testTag("trackSave")) { Text(stringResource(R.string.rec_save)) }
            } else {
                Button(onClick = onDiscard) { Text(stringResource(R.string.rec_done)) }
            }
        },
        dismissButton = { if (canSave) TextButton(onClick = onDiscard) { Text(stringResource(R.string.rec_discard)) } },
    )
}
