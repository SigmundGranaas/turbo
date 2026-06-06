package com.sigmundgranaas.turbo.expressive.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

/**
 * The standard Expressive surface card: a rounded `surfaceContainerHigh` panel
 * with internal padding. One definition so every screen's cards match.
 */
@Composable
fun TurboCard(
    modifier: Modifier = Modifier,
    color: Color = MaterialTheme.colorScheme.surfaceContainerHigh,
    padding: androidx.compose.ui.unit.Dp = 18.dp,
    onClick: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val base = modifier
        .fillMaxWidth()
        .clip(RoundedCornerShape(TurboRadius.xl))
        .background(color)
    Column(
        modifier = if (onClick != null) base.clickable(onClick = onClick).padding(padding) else base.padding(padding),
        content = content,
    )
}

/**
 * A compact metric tile: big value + small label, optional leading icon. Used in
 * the stat strips on path/activity detail screens.
 */
@Composable
fun StatTile(
    value: String,
    label: String,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
    accent: Color = MaterialTheme.colorScheme.primary,
) {
    val cs = MaterialTheme.colorScheme
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(TurboRadius.l))
            .background(cs.surfaceContainerHigh)
            .padding(vertical = 14.dp, horizontal = 14.dp)
            .clearAndSetSemantics { contentDescription = "$label: $value" },
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        if (icon != null) {
            Icon(icon, null, tint = accent, modifier = Modifier.size(20.dp))
            Spacer(Modifier.height(2.dp))
        }
        Text(value, style = MaterialTheme.typography.titleLarge, color = cs.onSurface)
        Text(label.uppercase(), style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant)
    }
}

/** Lays out a row of [StatTile]s with equal weight and the standard gap. */
@Composable
fun StatRow(modifier: Modifier = Modifier, content: @Composable RowScope.() -> Unit) {
    Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp), content = content)
}

/** A key→value line for spec/detail lists. */
@Composable
fun SpecRow(label: String, value: String, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Row(
        modifier.fillMaxWidth().padding(vertical = 8.dp).clearAndSetSemantics { contentDescription = "$label, $value" },
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant)
        Text(value, style = MaterialTheme.typography.titleSmall, color = cs.onSurface)
    }
}
