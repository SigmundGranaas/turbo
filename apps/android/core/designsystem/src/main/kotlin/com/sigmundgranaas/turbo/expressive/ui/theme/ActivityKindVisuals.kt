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
import androidx.annotation.StringRes
import androidx.compose.ui.graphics.vector.ImageVector
import com.sigmundgranaas.turbo.expressive.core.designsystem.R
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

/**
 * The localized display label for each activity kind. Kept here (not on the pure
 * [ActivityKindId]) so the model stays Compose/resource-free; resolve it with
 * `stringResource(kind.labelRes)` in composables, or `context.getString(kind.labelRes)`
 * off the UI thread.
 */
@get:StringRes
val ActivityKindId.labelRes: Int
    get() = when (this) {
        ActivityKindId.Mountain -> R.string.kind_mountain
        ActivityKindId.Park -> R.string.kind_park
        ActivityKindId.Beach -> R.string.kind_beach
        ActivityKindId.Forest -> R.string.kind_forest
        ActivityKindId.Hiking -> R.string.kind_hiking
        ActivityKindId.Kayaking -> R.string.kind_kayaking
        ActivityKindId.Biking -> R.string.kind_biking
        ActivityKindId.Cabin -> R.string.kind_cabin
        ActivityKindId.Parking -> R.string.kind_parking
        ActivityKindId.Camping -> R.string.kind_camping
        ActivityKindId.Swimming -> R.string.kind_swimming
        ActivityKindId.Diving -> R.string.kind_diving
        ActivityKindId.Viewpoint -> R.string.kind_viewpoint
        ActivityKindId.Restaurant -> R.string.kind_restaurant
        ActivityKindId.Cafe -> R.string.kind_cafe
        ActivityKindId.Accommodation -> R.string.kind_accommodation
        ActivityKindId.Fishing -> R.string.kind_fishing
        ActivityKindId.Skiing -> R.string.kind_skiing
    }
