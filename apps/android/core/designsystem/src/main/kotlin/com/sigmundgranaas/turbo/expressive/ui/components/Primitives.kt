package com.sigmundgranaas.turbo.expressive.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Explore
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material.icons.rounded.Lock
import androidx.compose.material.icons.rounded.LockOpen
import androidx.compose.material.icons.rounded.Menu
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material.icons.rounded.NearMe
import androidx.compose.material.icons.rounded.Remove
import androidx.compose.material.icons.rounded.Water
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
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
    bearing: Float = 0f,
    onCompass: (() -> Unit)? = null,
    /** Compass long-press → "Lock rotation" toggle; null hides the mini-menu. */
    rotationLocked: Boolean = false,
    onToggleRotationLock: (() -> Unit)? = null,
    onLayers: () -> Unit = {},
    onLocate: () -> Unit = {},
    onZoomIn: () -> Unit = {},
    onZoomOut: () -> Unit = {},
) {
    // The five-button rail (spec Phase 1): Compass (auto-hide) · Layers · Location
    // · + · −. Add-Marker + Route relocated to the quick-actions card; 3D + Sun to
    // the layers-sheet sliders. Nothing else belongs on the rail.
    Column(modifier = modifier, horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(10.dp)) {
        // Compass — shows only while the map is rotated off north (tap resets to north,
        // long-press toggles rotation lock). Hidden at north — including locked + north
        // (a locked map can't drift off north, so there's nothing to reset). The durable
        // unlock lives in Settings → Gestures so hiding here never traps the lock on.
        if (onCompass != null && compassVisible(bearing)) {
            CompassButton(
                bearing = bearing,
                rotationLocked = rotationLocked,
                onCompass = onCompass,
                onToggleRotationLock = onToggleRotationLock,
            )
        }
        RailButton(Icons.Rounded.Layers, stringResource(R.string.ds_map_layers), onClick = onLayers)
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

/**
 * Compass auto-hide rule (pure → unit-tested): the widget shows only when the map is
 * rotated more than ~0.5° off north. Rotation-lock does not change this — a locked map
 * simply can't drift off north, so it stays hidden (the spec's "locked + north ⇒ hidden").
 */
fun compassVisible(bearingDeg: Float): Boolean = kotlin.math.abs(bearingDeg) > 0.5f

/**
 * Rail compass: a needle that rotates with the bearing behind a subtle field-of-view wedge
 * (cosmetic heading cue). Tap resets to north; long-press opens a mini-menu with the
 * persisted "Lock rotation" toggle (see [MapControlRail]). A small padlock badge marks the
 * locked state.
 */
@Composable
private fun CompassButton(
    bearing: Float,
    rotationLocked: Boolean,
    onCompass: () -> Unit,
    onToggleRotationLock: (() -> Unit)?,
) {
    val interactionSource = remember { MutableInteractionSource() }
    var menuOpen by remember { mutableStateOf(false) }
    val wedge = MaterialTheme.colorScheme.primary
    Box {
        Surface(
            shape = RoundedCornerShape(18.dp),
            color = if (rotationLocked) MaterialTheme.colorScheme.tertiaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh,
            shadowElevation = 3.dp,
            modifier = Modifier
                .size(52.dp)
                .pressScale(interactionSource)
                .combinedClickable(
                    interactionSource = interactionSource,
                    indication = ripple(),
                    onClick = onCompass,
                    onLongClick = if (onToggleRotationLock != null) ({ menuOpen = true }) else null,
                ),
        ) {
            Box(contentAlignment = Alignment.Center) {
                // FOV wedge behind the needle — points where the map's "up" faces. Cosmetic.
                Canvas(modifier = Modifier.size(30.dp).rotate(-bearing)) {
                    val w = size.width
                    val h = size.height
                    val p = Path().apply {
                        moveTo(w / 2f, h * 0.52f)
                        lineTo(w * 0.32f, h * 0.04f)
                        lineTo(w * 0.68f, h * 0.04f)
                        close()
                    }
                    drawPath(p, color = wedge.copy(alpha = 0.28f))
                }
                Icon(
                    imageVector = Icons.Rounded.Explore,
                    contentDescription = stringResource(R.string.ds_compass),
                    tint = if (rotationLocked) MaterialTheme.colorScheme.onTertiaryContainer else MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(24.dp).rotate(-bearing),
                )
                if (rotationLocked) {
                    Icon(
                        imageVector = Icons.Rounded.Lock,
                        contentDescription = stringResource(R.string.ds_rotation_locked),
                        tint = MaterialTheme.colorScheme.onTertiaryContainer,
                        modifier = Modifier.align(Alignment.BottomEnd).offset(x = (-4).dp, y = (-4).dp).size(13.dp),
                    )
                }
            }
        }
        DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.ds_lock_rotation)) },
                onClick = {
                    onToggleRotationLock?.invoke()
                    menuOpen = false
                },
                leadingIcon = {
                    Icon(if (rotationLocked) Icons.Rounded.Lock else Icons.Rounded.LockOpen, contentDescription = null)
                },
                trailingIcon = { if (rotationLocked) Icon(Icons.Rounded.Check, contentDescription = null) },
            )
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
    val interactionSource = remember { MutableInteractionSource() }
    Surface(
        shape = RoundedCornerShape(18.dp),
        color = if (active) MaterialTheme.colorScheme.tertiaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh,
        shadowElevation = 3.dp,
        onClick = onClick,
        interactionSource = interactionSource,
        modifier = Modifier.size(52.dp).pressScale(interactionSource),
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
    val interactionSource = remember { MutableInteractionSource() }
    Surface(
        shape = CircleShape,
        color = Color.Transparent,
        onClick = onClick,
        interactionSource = interactionSource,
        modifier = modifier.size(size).pressScale(interactionSource),
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
