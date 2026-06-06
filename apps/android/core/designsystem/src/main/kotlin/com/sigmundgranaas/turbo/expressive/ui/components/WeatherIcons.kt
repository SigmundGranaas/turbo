package com.sigmundgranaas.turbo.expressive.ui.components

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AcUnit
import androidx.compose.material.icons.rounded.Air
import androidx.compose.material.icons.rounded.BlurOn
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.Grain
import androidx.compose.material.icons.rounded.Navigation
import androidx.compose.material.icons.rounded.Thunderstorm
import androidx.compose.material.icons.rounded.Umbrella
import androidx.compose.material.icons.rounded.WbCloudy
import androidx.compose.material.icons.rounded.WbSunny
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.res.stringResource
import com.sigmundgranaas.turbo.expressive.core.designsystem.R
import com.sigmundgranaas.turbo.expressive.domain.WeatherKind
import com.sigmundgranaas.turbo.expressive.domain.classifyWeatherSymbol

/**
 * A Material glyph for a MET weather symbol code. We map to the built-in icon set
 * rather than bundling met.no's ~90 SVGs — recognisable, themeable, zero assets.
 */
fun weatherIcon(symbolCode: String?): ImageVector = when (classifyWeatherSymbol(symbolCode)) {
    WeatherKind.Clear -> Icons.Rounded.WbSunny
    WeatherKind.PartlyCloudy -> Icons.Rounded.WbCloudy
    WeatherKind.Cloudy -> Icons.Rounded.Cloud
    WeatherKind.Fog -> Icons.Rounded.BlurOn
    WeatherKind.Rain -> Icons.Rounded.Umbrella
    WeatherKind.Sleet -> Icons.Rounded.Grain
    WeatherKind.Snow -> Icons.Rounded.AcUnit
    WeatherKind.Thunder -> Icons.Rounded.Thunderstorm
    WeatherKind.Unknown -> Icons.Rounded.Cloud
}

/**
 * An arrow pointing the way the wind blows *towards* (MET reports the direction it
 * comes *from*, so we add 180°). Null bearing renders a neutral breeze glyph.
 */
@Composable
fun WindArrow(
    fromDegrees: Double?,
    modifier: Modifier = Modifier,
    tint: Color = MaterialTheme.colorScheme.onSurfaceVariant,
) {
    if (fromDegrees == null) {
        Icon(Icons.Rounded.Air, contentDescription = stringResource(R.string.ds_wind), tint = tint, modifier = modifier)
        return
    }
    // Navigation arrow points up (north / towards) by default; rotate to the "to" heading.
    Icon(
        Icons.Rounded.Navigation,
        contentDescription = stringResource(R.string.ds_wind_direction),
        tint = tint,
        modifier = modifier.graphicsLayer { rotationZ = ((fromDegrees + 180.0) % 360.0).toFloat() },
    )
}
