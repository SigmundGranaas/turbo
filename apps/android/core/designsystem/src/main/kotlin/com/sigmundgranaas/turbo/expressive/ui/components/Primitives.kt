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
import androidx.compose.material.icons.rounded.Explore
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material.icons.rounded.Menu
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material.icons.rounded.NearMe
import androidx.compose.material.icons.rounded.Remove
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

/** Section eyebrow: uppercase, tracked, primary/onSurfaceVariant. */
@Composable
fun SectionLabel(text: String, modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.onSurfaceVariant) {
    Text(
        text = text.uppercase(),
        style = MaterialTheme.typography.labelSmall,
        color = color,
        modifier = modifier,
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
            IconBtn(Icons.Rounded.Menu, "Menu", onClick = onMenuClick, tint = MaterialTheme.colorScheme.onSurface)
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
    compassOn: Boolean = false,
    compassRotation: Float = 0f,
    onLayers: () -> Unit = {},
    onLocate: () -> Unit = {},
    onCompass: () -> Unit = {},
    onZoomIn: () -> Unit = {},
    onZoomOut: () -> Unit = {},
) {
    Column(modifier = modifier, horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(10.dp)) {
        RailButton(Icons.Rounded.Layers, "Map layers", onClick = onLayers)
        RailButton(
            icon = if (following) Icons.Rounded.MyLocation else Icons.Rounded.NearMe,
            desc = "My location",
            active = following,
            onClick = onLocate,
        )
        RailButton(Icons.Rounded.Explore, "Compass", active = compassOn, rotation = -compassRotation, onClick = onCompass)
        Surface(
            shape = CircleShape,
            color = MaterialTheme.colorScheme.surfaceContainerHigh,
            shadowElevation = 3.dp,
        ) {
            Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.padding(vertical = 4.dp)) {
                IconBtn(Icons.Rounded.Add, "Zoom in", size = 42.dp, onClick = onZoomIn)
                HorizontalDivider(modifier = Modifier.width(28.dp), color = MaterialTheme.colorScheme.outlineVariant)
                IconBtn(Icons.Rounded.Remove, "Zoom out", size = 42.dp, onClick = onZoomOut)
            }
        }
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
        Column(Modifier.weight(1f)) {
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
