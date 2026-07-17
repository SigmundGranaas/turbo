package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AddAPhoto
import androidx.compose.material.icons.rounded.AddLocationAlt
import androidx.compose.material.icons.rounded.Navigation
import androidx.compose.material.icons.rounded.Place
import androidx.compose.material.icons.rounded.Route
import androidx.compose.material.icons.rounded.Straighten
import androidx.compose.material.icons.rounded.TripOrigin
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.core.geo.formatCoords
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.feature.conditions.ConditionsUiState
import com.sigmundgranaas.turbo.expressive.feature.conditions.ConditionsViewModel
import com.sigmundgranaas.turbo.expressive.ui.components.pressScaleClickable
import com.sigmundgranaas.turbo.expressive.ui.components.weatherIcon
import kotlin.math.roundToInt

/**
 * The on-map contextual menu that blooms where you long-press — an M3 Expressive
 * surface anchored at the touch point with a ghost pin, a tappable weather readout
 * for that spot, and the create actions as a 2×2 grid of equal, labelled tiles. It
 * scales+fades in from the press point and back out via [visibleState] (kept mounted
 * by the caller through the exit).
 */
@Composable
internal fun MapLongPressMenu(
    visibleState: MutableTransitionState<Boolean>,
    point: LatLng,
    anchor: Offset,
    onNewMarker: () -> Unit,
    onRouteHere: () -> Unit,
    /** Begin building a route whose FIRST waypoint is this point (drops an origin pin). */
    onStartRouteHere: () -> Unit,
    onCreateTrack: () -> Unit,
    onAddPhoto: () -> Unit,
    /** Measure from this point (Line mode). Null when offline — the row shows a
     *  disabled "not available offline" state instead of opening a dead tool. */
    onMeasure: (() -> Unit)?,
    onDismiss: () -> Unit,
    /** Tapping the weather readout opens the full forecast for this point. */
    onOpenForecast: () -> Unit,
    /** Reverse-geocoded place label ("On Storfjellet, 612 m"); falls back to coords. */
    placeLabel: String? = null,
    conditionsViewModel: ConditionsViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val density = LocalDensity.current
    LaunchedEffect(point) { conditionsViewModel.load(point) }
    val conditions by conditionsViewModel.state.collectAsStateWithLifecycle()

    androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxSize()) {
        // Scrim — fades with the menu; tap anywhere off to dismiss.
        AnimatedVisibility(visibleState = visibleState, enter = fadeIn(), exit = fadeOut()) {
            Box(
                Modifier.fillMaxSize()
                    .background(cs.scrim.copy(alpha = 0.32f))
                    .testTag("lpScrim")
                    .clickable(
                        interactionSource = remember { androidx.compose.foundation.interaction.MutableInteractionSource() },
                        indication = null,
                        onClick = onDismiss,
                    ),
            )
        }

        // Ghost pin at the press point (tip sits on the coordinate).
        AnimatedVisibility(
            visibleState = visibleState,
            enter = fadeIn() + scaleIn(initialScale = 0.7f, transformOrigin = TransformOrigin(0.5f, 1f)),
            exit = fadeOut(),
            modifier = Modifier.offset {
                IntOffset((anchor.x - with(density) { 18.dp.toPx() }).roundToInt(), (anchor.y - with(density) { 36.dp.toPx() }).roundToInt())
            },
        ) {
            Icon(Icons.Rounded.Place, null, tint = cs.primary, modifier = Modifier.size(36.dp))
        }

        // Card: clamp horizontally; sit below the pin, but lift up near the bottom edge.
        val cardWidth = 280.dp
        val margin = 12.dp
        val cardWidthPx = with(density) { cardWidth.toPx() }
        val marginPx = with(density) { margin.toPx() }
        // Realistic card height (mini-weather + 5 action rows); errs tall so the clamp
        // never lets the bottom action slip under the gesture-nav bar.
        val estCardHeightPx = with(density) { 488.dp.toPx() }
        val navBottomPx = WindowInsets.navigationBars.getBottom(density).toFloat()
        val statusTopPx = WindowInsets.statusBars.getTop(density).toFloat()
        val maxW = constraints.maxWidth.toFloat()
        val maxH = constraints.maxHeight.toFloat()
        val left = (anchor.x - cardWidthPx / 2f).coerceIn(marginPx, (maxW - cardWidthPx - marginPx).coerceAtLeast(marginPx))
        val topMin = statusTopPx + marginPx
        val topMax = (maxH - navBottomPx - estCardHeightPx - marginPx).coerceAtLeast(topMin)
        val top = (anchor.y + with(density) { 28.dp.toPx() }).coerceIn(topMin, topMax)
        // Bloom from the press point: grow toward where the pin sits relative to the card.
        val originX = ((anchor.x - left) / cardWidthPx).coerceIn(0f, 1f)
        val originY = if (top < anchor.y) 1f else 0f
        val spatial = MaterialTheme.motionScheme.fastSpatialSpec<Float>()

        AnimatedVisibility(
            visibleState = visibleState,
            enter = scaleIn(animationSpec = spatial, initialScale = 0.85f, transformOrigin = TransformOrigin(originX, originY)) +
                fadeIn(MaterialTheme.motionScheme.defaultEffectsSpec()),
            exit = scaleOut(animationSpec = spatial, targetScale = 0.85f, transformOrigin = TransformOrigin(originX, originY)) +
                fadeOut(MaterialTheme.motionScheme.defaultEffectsSpec()),
            modifier = Modifier.offset { IntOffset(left.roundToInt(), top.roundToInt()) }.width(cardWidth),
        ) {
            Surface(
                shape = RoundedCornerShape(28.dp),
                color = cs.surfaceContainerHigh,
                shadowElevation = 6.dp,
                modifier = Modifier.testTag("lpMenu"),
            ) {
                Column(Modifier.padding(12.dp)) {
                    MiniWeather(conditions, point, placeLabel, onClick = onOpenForecast)
                    Spacer(Modifier.height(10.dp))
                    // A simple list of actions — each rises + fades in with a slight stagger
                    // so the menu feels alive, and springs on press.
                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        ActionRow(Icons.Rounded.AddLocationAlt, stringResource(R.string.lp_new_marker), onNewMarker, "lpNewMarker", index = 0, primary = true)
                        ActionRow(Icons.Rounded.Navigation, stringResource(R.string.lp_route_here), onRouteHere, "lpRouteHere", index = 1)
                        ActionRow(Icons.Rounded.TripOrigin, stringResource(R.string.lp_start_route), onStartRouteHere, "lpStartRoute", index = 2)
                        ActionRow(Icons.Rounded.Route, stringResource(R.string.track_title), onCreateTrack, "lpCreateTrack", index = 3)
                        ActionRow(Icons.Rounded.AddAPhoto, stringResource(R.string.lp_add_photo), onAddPhoto, "lpAddPhoto", index = 4)
                        if (onMeasure != null) {
                            ActionRow(Icons.Rounded.Straighten, stringResource(R.string.lp_measure), onMeasure, "lpMeasure", index = 5)
                        } else {
                            ActionRow(
                                Icons.Rounded.Straighten,
                                stringResource(R.string.lp_measure_offline),
                                onClick = {},
                                tag = "lpMeasureOffline",
                                index = 5,
                                enabled = false,
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun MiniWeather(state: ConditionsUiState, point: LatLng, placeLabel: String?, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    val where = placeLabel ?: formatCoords(point)
    // One fixed layout for every state — a 28 dp leading slot + a two-line column — so the
    // row is exactly as tall while loading as it is once the weather arrives. Only the
    // *contents* of the slots swap, so nothing below shifts when Loading → Content.
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth()
            .clip(RoundedCornerShape(18.dp))
            .background(cs.surfaceContainerLow)
            .pressScaleClickable(onClick = onClick)
            .testTag("lpWeather")
            .padding(horizontal = 14.dp, vertical = 10.dp),
    ) {
        // Leading 28 dp slot: weather glyph, a spinner while loading, or the fallback glyph.
        Box(Modifier.size(28.dp), contentAlignment = Alignment.Center) {
            when (state) {
                is ConditionsUiState.Content ->
                    Icon(weatherIcon(state.conditions.weather?.symbolCode), null, tint = cs.primary, modifier = Modifier.size(28.dp))
                ConditionsUiState.Loading ->
                    CircularProgressIndicator(modifier = Modifier.size(18.dp), strokeWidth = 2.dp)
                ConditionsUiState.Error ->
                    Icon(weatherIcon(null), null, tint = cs.onSurfaceVariant, modifier = Modifier.size(24.dp))
            }
        }
        Spacer(Modifier.width(10.dp))
        // Primary line is always titleMedium and the secondary always labelSmall, so the
        // column height is identical regardless of state.
        Column(Modifier.weight(1f)) {
            val primary = when (state) {
                is ConditionsUiState.Content ->
                    state.conditions.weather?.temperatureC?.let { "${it.roundToInt()}°" } ?: "—"
                ConditionsUiState.Loading -> stringResource(R.string.cond_header)
                ConditionsUiState.Error -> "—"
            }
            Text(
                primary,
                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.W700),
                color = if (state is ConditionsUiState.Content) cs.onSurface else cs.onSurfaceVariant,
                maxLines = 1,
            )
            Text(where, style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant, maxLines = 1)
        }
        if (state is ConditionsUiState.Content) {
            state.conditions.avalanche?.dangerLevel?.takeIf { it >= 3 }?.let { lvl ->
                Text("⚠ $lvl", style = MaterialTheme.typography.labelMedium, color = cs.error)
            }
        }
    }
}

/** One create-action as a full-width list row (icon-in-circle + label). Rises + fades
 *  in with a per-[index] stagger when the menu blooms, and springs on press. */
@Composable
private fun ActionRow(
    icon: ImageVector,
    label: String,
    onClick: () -> Unit,
    tag: String,
    index: Int,
    primary: Boolean = false,
    enabled: Boolean = true,
) {
    val cs = MaterialTheme.colorScheme
    val container = when {
        !enabled -> cs.surfaceContainerHighest.copy(alpha = 0.5f)
        primary -> cs.secondaryContainer
        else -> cs.surfaceContainerHighest
    }
    val onContainer = when {
        !enabled -> cs.onSurfaceVariant.copy(alpha = 0.6f)
        primary -> cs.onSecondaryContainer
        else -> cs.onSurface
    }
    // Drive the staggered entrance: start hidden on first composition, then animate in.
    var appeared by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { appeared = true }
    val progress by animateFloatAsState(
        targetValue = if (appeared) 1f else 0f,
        animationSpec = tween(durationMillis = 300, delayMillis = index * 45, easing = FastOutSlowInEasing),
        label = "lpRowEnter",
    )
    Row(
        Modifier
            .fillMaxWidth()
            .graphicsLayer { alpha = progress; translationY = (1f - progress) * 18.dp.toPx() }
            .clip(RoundedCornerShape(18.dp))
            .background(container)
            .pressScaleClickable(onClick = onClick, onClickLabel = label)
            .testTag(tag)
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier.size(40.dp).clip(CircleShape).background(cs.primary.copy(alpha = 0.14f)),
            contentAlignment = Alignment.Center,
        ) {
            Icon(icon, null, tint = cs.primary, modifier = Modifier.size(22.dp))
        }
        Spacer(Modifier.width(14.dp))
        Text(
            label,
            style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.W600),
            color = onContainer,
        )
    }
}
