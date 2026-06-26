package com.sigmundgranaas.turbo.expressive.feature.map.route

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Balance
import androidx.compose.material.icons.rounded.Forest
import androidx.compose.material.icons.rounded.Hiking
import androidx.compose.material.icons.rounded.Straight
import androidx.compose.material.icons.rounded.Terrain
import androidx.compose.ui.graphics.vector.ImageVector
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset

/** A glyph that conveys each route style at a glance, used in the preset picker. */
val RoutePreset.icon: ImageVector
    get() = when (this) {
        RoutePreset.Balanced -> Icons.Rounded.Balance
        RoutePreset.AvoidRoads -> Icons.Rounded.Forest
        RoutePreset.Direct -> Icons.Rounded.Straight
        RoutePreset.EasyGrade -> Icons.Rounded.Terrain
        RoutePreset.TrailPurist -> Icons.Rounded.Hiking
    }
