package com.sigmundgranaas.turbo.expressive.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.AddLocationAlt
import androidx.compose.material.icons.rounded.Explore
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material.icons.rounded.Route
import androidx.compose.material.icons.rounded.Menu
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material.icons.rounded.NearMe
import androidx.compose.material.icons.rounded.Remove
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.core.designsystem.R

/** Section eyebrow: uppercase, tracked, primary/onSurfaceVariant. */
@Composable
fun SectionLabel(text: String, modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.onSurfaceVariant) {
    Text(
        text = text.uppercase(),
        style = MaterialTheme.typography.labelSmall,
        color = color,
        modifier = modifier.semantics { heading() },
    )
}

/** Docked Expressive search pill: leading menu, hint/value, trailing avatar. */
@Composable
fun SearchPill(
    placeholder: String,
    modifier: Modifier = Modifier,
    value: String? = null,
    avatarInitial: String = "S",
    onMenuClick: () -> Unit = {},
    onClick: () -> Unit = {},
) {
    Surface(
        modifier = modifier.fillMaxWidth().height(56.dp),
        shape = CircleShape,
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        shadowElevation = 4.dp,
        onClick = onClick,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 6.dp, end = 8.dp)) {
            IconBtn(Icons.Rounded.Menu, stringResource(R.string.ds_menu), onClick = onMenuClick, tint = MaterialTheme.colorScheme.onSurface)
            Text(
                text = value ?: placeholder,
                style = MaterialTheme.typography.bodyLarge,
                color = if (value != null) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f).padding(horizontal = 4.dp),
            )
            Box(
                modifier = Modifier.size(36.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary),
                contentAlignment = Alignment.Center,
            ) {
                Text(avatarInitial, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onPrimary)
            }
        }
    }
}

/** Vertical map-control rail: layers · locate · compass + a zoom group. */
@Composable
fun MapControlRail(
    modifier: Modifier = Modifier,
    following: Boolean = false,
    creatingTrack: Boolean = false,
    bearing: Float = 0f,
    onCompass: (() -> Unit)? = null,
    onAdd: (() -> Unit)? = null,
    onLayers: () -> Unit = {},
    onLocate: () -> Unit = {},
    onCreateTrack: (() -> Unit)? = null,
    onZoomIn: () -> Unit = {},
    onZoomOut: () -> Unit = {},
) {
    Column(modifier = modifier, horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(10.dp)) {
        // Discoverable "add a place" — drops a new marker at the map centre. Long-press
        // anywhere is the shortcut; this is the visible door for it.
        if (onAdd != null) {
            RailButton(Icons.Rounded.AddLocationAlt, stringResource(R.string.ds_add_place), active = true, onClick = onAdd)
        }
        // Compass — only while the map is rotated; tap resets to north. Lives here
        // (in the inset rail) instead of MapLibre's off-screen default widget.
        if (onCompass != null && kotlin.math.abs(bearing) > 0.5f) {
            RailButton(Icons.Rounded.Explore, stringResource(R.string.ds_compass), rotation = -bearing, onClick = onCompass)
        }
        RailButton(Icons.Rounded.Layers, stringResource(R.string.ds_map_layers), onClick = onLayers)
        if (onCreateTrack != null) {
            RailButton(Icons.Rounded.Route, stringResource(R.string.ds_create_track), active = creatingTrack, onClick = onCreateTrack)
        }
        RailButton(
            icon = if (following) Icons.Rounded.MyLocation else Icons.Rounded.NearMe,
            desc = stringResource(R.string.ds_my_location),
            active = following,
            onClick = onLocate,
        )
        // Zoom in/out as two standalone cookie buttons, matching the rail above —
        // not a divided pill, which read as a different control entirely.
        RailButton(Icons.Rounded.Add, stringResource(R.string.ds_zoom_in), onClick = onZoomIn)
        RailButton(Icons.Rounded.Remove, stringResource(R.string.ds_zoom_out), onClick = onZoomOut)
    }
}

@Composable
private fun RailButton(
    icon: ImageVector,
    desc: String,
    active: Boolean = false,
    rotation: Float = 0f,
    onClick: () -> Unit,
) {
    Surface(
        shape = RoundedCornerShape(18.dp),
        color = if (active) MaterialTheme.colorScheme.tertiaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh,
        shadowElevation = 3.dp,
        onClick = onClick,
        modifier = Modifier.size(52.dp),
    ) {
        Box(contentAlignment = Alignment.Center) {
            Icon(
                imageVector = icon,
                contentDescription = desc,
                tint = if (active) MaterialTheme.colorScheme.onTertiaryContainer else MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(24.dp).rotate(rotation),
            )
        }
    }
}

/** A bare circular icon button (no container), used inside pills/rows. */
@Composable
fun IconBtn(
    icon: ImageVector,
    desc: String,
    modifier: Modifier = Modifier,
    size: androidx.compose.ui.unit.Dp = 48.dp,
    tint: Color = MaterialTheme.colorScheme.onSurfaceVariant,
    onClick: () -> Unit = {},
) {
    Surface(
        shape = CircleShape,
        color = Color.Transparent,
        onClick = onClick,
        modifier = modifier.size(size),
    ) {
        Box(contentAlignment = Alignment.Center) {
            Icon(icon, desc, tint = tint, modifier = Modifier.size(24.dp))
        }
    }
}

/** Generic list row: leading icon circle, title + optional subtitle, trailing slot. */
@Composable
fun ListRowItem(
    icon: ImageVector,
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    iconTone: RowTone = RowTone.Surface,
    trailing: @Composable (() -> Unit)? = null,
) {
    val (bg, fg) = when (iconTone) {
        RowTone.Primary -> MaterialTheme.colorScheme.primaryContainer to MaterialTheme.colorScheme.onPrimaryContainer
        RowTone.Surface -> MaterialTheme.colorScheme.surfaceContainerHigh to MaterialTheme.colorScheme.primary
    }
    Row(verticalAlignment = Alignment.CenterVertically, modifier = modifier.fillMaxWidth().padding(vertical = 12.dp)) {
        Box(modifier = Modifier.size(44.dp).clip(CircleShape).background(bg), contentAlignment = Alignment.Center) {
            Icon(icon, null, tint = fg, modifier = Modifier.size(22.dp))
        }
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f).semantics(mergeDescendants = true) {}) {
            Text(title, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (subtitle != null) {
                Text(subtitle, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        if (trailing != null) {
            Spacer(Modifier.width(12.dp))
            trailing()
        }
    }
}

enum class RowTone { Primary, Surface }
