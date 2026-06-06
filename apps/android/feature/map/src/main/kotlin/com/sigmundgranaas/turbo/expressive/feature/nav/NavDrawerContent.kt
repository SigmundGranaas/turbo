package com.sigmundgranaas.turbo.expressive.feature.nav

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CloudDownload
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.FiberManualRecord
import androidx.compose.material.icons.rounded.Map
import androidx.compose.material.icons.rounded.Route
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.NavigationDrawerItemDefaults
import androidx.compose.material3.Text
import androidx.annotation.StringRes
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.feature.map.R
import com.sigmundgranaas.turbo.expressive.ui.components.Cookie
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius

enum class DrawerDestination(@StringRes val labelRes: Int, val icon: androidx.compose.ui.graphics.vector.ImageVector) {
    Map(R.string.drawer_map, Icons.Rounded.Map),
    Paths(R.string.drawer_paths, Icons.Rounded.Route),
    Collections(R.string.drawer_collections, Icons.Rounded.Folder),
    Record(R.string.drawer_record, Icons.Rounded.FiberManualRecord),
    Offline(R.string.drawer_offline, Icons.Rounded.CloudDownload),
    Settings(R.string.drawer_settings, Icons.Rounded.Settings),
}

@Composable
fun NavDrawerContent(
    selected: DrawerDestination,
    onSelect: (DrawerDestination) -> Unit,
) {
    val cs = MaterialTheme.colorScheme
    ModalDrawerSheet(
        drawerContainerColor = cs.surfaceContainerLow,
        drawerShape = RoundedCornerShape(topEnd = TurboRadius.xxl, bottomEnd = TurboRadius.xxl),
    ) {
        Spacer(Modifier.height(28.dp))
        Text("Turbo", style = MaterialTheme.typography.headlineMedium, color = cs.onSurface, modifier = Modifier.padding(horizontal = 28.dp))
        Text(stringResource(R.string.drawer_tagline), style = MaterialTheme.typography.bodyMedium, color = cs.onSurfaceVariant, modifier = Modifier.padding(horizontal = 28.dp))

        Spacer(Modifier.height(18.dp))
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(horizontal = 16.dp).fillMaxWidth()
                .background(cs.primaryContainer, RoundedCornerShape(TurboRadius.l)).padding(12.dp),
        ) {
            Cookie(size = 42.dp, fill = cs.surface) {
                Text("S", style = MaterialTheme.typography.titleMedium, color = cs.onPrimaryContainer)
            }
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text("Sigmund G.", style = MaterialTheme.typography.titleSmall, color = cs.onPrimaryContainer, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text("sigmund@turkart.no", style = MaterialTheme.typography.bodySmall, color = cs.onPrimaryContainer)
            }
        }

        Spacer(Modifier.height(12.dp))
        DrawerDestination.entries.forEach { dest ->
            NavigationDrawerItem(
                icon = { Icon(dest.icon, null) },
                label = { Text(stringResource(dest.labelRes)) },
                selected = dest == selected,
                onClick = { onSelect(dest) },
                colors = NavigationDrawerItemDefaults.colors(
                    selectedContainerColor = cs.secondaryContainer,
                    selectedTextColor = cs.onSecondaryContainer,
                    selectedIconColor = cs.onSecondaryContainer,
                ),
                modifier = Modifier.padding(horizontal = 14.dp, vertical = 2.dp),
            )
        }
    }
}
