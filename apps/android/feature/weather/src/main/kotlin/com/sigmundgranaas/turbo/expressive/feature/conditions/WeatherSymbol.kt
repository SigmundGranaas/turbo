package com.sigmundgranaas.turbo.expressive.feature.conditions

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ProvidableCompositionLocal
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.ImageLoader
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import coil3.svg.SvgDecoder

/**
 * The real yr.no/met.no weather symbols — the same coloured SVG icon set the Flutter
 * app ships — rendered from `assets/weather/<code>.svg` via Coil's SVG decoder, so the
 * forecast reads as proper weather glyphs (sun, cloud, rain) instead of flat monochrome
 * Material icons. Falls back to `cloudy.svg` for unknown/missing codes.
 */
@Composable
fun WeatherSymbol(code: String?, modifier: Modifier = Modifier, size: Dp = 28.dp) {
    val context = LocalContext.current
    val loader = LocalWeatherImageLoader.current
        ?: remember(context) { weatherImageLoader(context) }
    val available = remember(context) {
        runCatching {
            context.assets.list("weather")?.map { it.removeSuffix(".svg") }?.toSet()
        }.getOrNull().orEmpty()
    }
    val name = code?.takeIf { it in available } ?: "cloudy"
    AsyncImage(
        model = remember(name) {
            ImageRequest.Builder(context).data("file:///android_asset/weather/$name.svg").build()
        },
        imageLoader = loader,
        contentDescription = null,
        modifier = modifier.size(size),
    )
}

/** A Coil loader wired with the SVG decoder — share one across a forecast view. */
fun weatherImageLoader(context: android.content.Context): ImageLoader =
    ImageLoader.Builder(context).components { add(SvgDecoder.Factory()) }.build()

/** Provided once per forecast sheet so the many [WeatherSymbol]s reuse one loader. */
val LocalWeatherImageLoader: ProvidableCompositionLocal<ImageLoader?> =
    staticCompositionLocalOf { null }

/** Provide a shared [weatherImageLoader] to all [WeatherSymbol]s in [content]. */
@Composable
fun ProvideWeatherImageLoader(content: @Composable () -> Unit) {
    val context = LocalContext.current
    val loader = remember(context) { weatherImageLoader(context) }
    CompositionLocalProvider(LocalWeatherImageLoader provides loader, content = content)
}
