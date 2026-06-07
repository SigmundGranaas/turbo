package com.sigmundgranaas.turbo.expressive.feature.conditions

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Thermostat
import androidx.compose.material.icons.rounded.Warning
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.feature.map.R
import com.sigmundgranaas.turbo.expressive.ui.theme.DangerColors
import kotlin.math.roundToInt

/**
 * Loads + shows the along-route conditions summary for [geometry]. Stateful shell
 * around [RouteConditionsContent]; safe to drop into the route card as a slot.
 */
@Composable
fun RouteConditionsStrip(
    geometry: List<LatLng>,
    modifier: Modifier = Modifier,
    viewModel: RouteConditionsViewModel = hiltViewModel(),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    androidx.compose.runtime.LaunchedEffect(geometry) { viewModel.load(geometry) }
    RouteConditionsContent(state, modifier)
}

/** Stateless render of the along-route conditions; renders nothing when there's nothing to say. */
@Composable
fun RouteConditionsContent(state: RouteConditionsUiState, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    when (state) {
        RouteConditionsUiState.Idle, RouteConditionsUiState.Error -> return
        RouteConditionsUiState.Loading -> {
            Text(
                stringResource(R.string.route_cond_loading),
                style = MaterialTheme.typography.labelMedium,
                color = cs.onSurfaceVariant,
                modifier = modifier.testTag("routeCondLoading"),
            )
        }
        is RouteConditionsUiState.Content -> {
            val summary = state.summary
            if (!summary.hasData) return
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
                modifier = modifier.testTag("routeCond"),
            ) {
                Text(
                    stringResource(R.string.route_cond_label),
                    style = MaterialTheme.typography.labelSmall,
                    color = cs.onSurfaceVariant,
                )
                summary.tempText()?.let { temp ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Rounded.Thermostat, null, tint = cs.primary, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(3.dp))
                        Text(temp, style = MaterialTheme.typography.labelLarge, color = cs.onSurface, modifier = Modifier.testTag("routeCondTemp"))
                    }
                }
                summary.worstDanger?.let { level ->
                    val danger = DangerColors.all[(level - 1).coerceIn(0, 4)]
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier
                            .background(danger.copy(alpha = 0.18f), RoundedCornerShape(50))
                            .padding(horizontal = 8.dp, vertical = 2.dp)
                            .testTag("routeCondAvalanche"),
                    ) {
                        Icon(Icons.Rounded.Warning, null, tint = danger, modifier = Modifier.size(14.dp))
                        Spacer(Modifier.width(4.dp))
                        Text(
                            stringResource(R.string.route_cond_avalanche, level),
                            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.W700),
                            color = cs.onSurface,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun RouteConditions.tempText(): String? {
    val lo = tempMinC?.roundToInt() ?: return null
    val hi = tempMaxC?.roundToInt() ?: return null
    return if (lo == hi) {
        stringResource(R.string.route_cond_temp_single, lo)
    } else {
        stringResource(R.string.route_cond_temp_range, lo, hi)
    }
}
