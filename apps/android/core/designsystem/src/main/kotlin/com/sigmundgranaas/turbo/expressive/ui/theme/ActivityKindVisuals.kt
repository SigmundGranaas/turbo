package com.sigmundgranaas.turbo.expressive.ui.theme

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.DirectionsBike
import androidx.compose.material.icons.rounded.AirportShuttle
import androidx.compose.material.icons.rounded.BeachAccess
import androidx.compose.material.icons.rounded.Cabin
import androidx.compose.material.icons.rounded.DownhillSkiing
import androidx.compose.material.icons.rounded.Forest
import androidx.compose.material.icons.rounded.Hiking
import androidx.compose.material.icons.rounded.Hotel
import androidx.compose.material.icons.rounded.Kayaking
import androidx.compose.material.icons.rounded.Landscape
import androidx.compose.material.icons.rounded.LocalCafe
import androidx.compose.material.icons.rounded.LocalParking
import androidx.compose.material.icons.rounded.Park
import androidx.compose.material.icons.rounded.Phishing
import androidx.compose.material.icons.rounded.PhotoCamera
import androidx.compose.material.icons.rounded.Pool
import androidx.compose.material.icons.rounded.Restaurant
import androidx.compose.material.icons.rounded.ScubaDiving
import androidx.compose.ui.graphics.vector.ImageVector
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId

/**
 * The Material Symbols Rounded glyph for each activity kind. Lives in the design
 * system (Compose) so `:core:model`'s [ActivityKindId] stays pure Kotlin.
 */
val ActivityKindId.icon: ImageVector
    get() = when (this) {
        ActivityKindId.Mountain -> Icons.Rounded.Landscape
        ActivityKindId.Park -> Icons.Rounded.Park
        ActivityKindId.Beach -> Icons.Rounded.BeachAccess
        ActivityKindId.Forest -> Icons.Rounded.Forest
        ActivityKindId.Hiking -> Icons.Rounded.Hiking
        ActivityKindId.Kayaking -> Icons.Rounded.Kayaking
        ActivityKindId.Biking -> Icons.AutoMirrored.Rounded.DirectionsBike
        ActivityKindId.Cabin -> Icons.Rounded.Cabin
        ActivityKindId.Parking -> Icons.Rounded.LocalParking
        ActivityKindId.Camping -> Icons.Rounded.AirportShuttle
        ActivityKindId.Swimming -> Icons.Rounded.Pool
        ActivityKindId.Diving -> Icons.Rounded.ScubaDiving
        ActivityKindId.Viewpoint -> Icons.Rounded.PhotoCamera
        ActivityKindId.Restaurant -> Icons.Rounded.Restaurant
        ActivityKindId.Cafe -> Icons.Rounded.LocalCafe
        ActivityKindId.Accommodation -> Icons.Rounded.Hotel
        ActivityKindId.Fishing -> Icons.Rounded.Phishing
        ActivityKindId.Skiing -> Icons.Rounded.DownhillSkiing
    }
