package com.sigmundgranaas.turbo.expressive.feature.map.live

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Cabin
import androidx.compose.material.icons.rounded.ChevronRight
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.ui.components.Cookie
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

/**
 * The "next waypoint" row shared between the follow lock card and the follow
 * sheet: a tertiary cookie cabin badge, a "Next waypoint · {distance}" caption,
 * the waypoint [name], and a chevron.
 */
@Composable
fun NextWaypointRow(
    caption: String,
    name: String,
    modifier: Modifier = Modifier,
) {
    val cs = MaterialTheme.colorScheme
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(TurboRadius.l))
            .background(cs.surfaceContainerHigh)
            .padding(horizontal = 12.dp, vertical = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Cookie(size = 36.dp, fill = cs.tertiary) {
            Icon(Icons.Rounded.Cabin, null, tint = cs.onTertiary, modifier = Modifier.size(19.dp))
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(caption, style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant)
            Text(
                name,
                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.W700),
                color = cs.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis,
            )
        }
        Icon(Icons.Rounded.ChevronRight, null, tint = cs.onSurfaceVariant, modifier = Modifier.size(22.dp))
    }
}
