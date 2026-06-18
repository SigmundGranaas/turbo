package com.sigmundgranaas.turbo.expressive.feature.map.radar

import com.sigmundgranaas.turbo.expressive.core.data.RadarRepository
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds

/**
 * Live cloud-overlay source backed by MET Norway. Delegates to [RadarRepository]
 * (which samples `locationforecast` over the viewport into a coverage/precip
 * grid sequence — see its docs) and adapts the domain
 * [com.sigmundgranaas.turbo.expressive.domain.RadarGridFrame]s to
 * [RadarFrameData].
 *
 * Falls back to the offline [SyntheticRadarDataSource] when the fetch fails or
 * returns nothing (no network, MET down), so the overlay always has something
 * to draw.
 */
class MetRadarDataSource(
    private val repo: RadarRepository,
    private val fallback: RadarDataSource = SyntheticRadarDataSource(),
) : RadarDataSource {

    override suspend fun load(bounds: GeoBounds, frameCount: Int): List<RadarFrameData> {
        val frames = runCatching { repo.forecastFrames(bounds, frameCount) }
            .getOrNull()
            .orEmpty()
        if (frames.isEmpty()) return fallback.load(bounds, frameCount)
        return frames.map {
            RadarFrameData(it.gridW, it.gridH, it.precip, it.coverage, it.epochMillis)
        }
    }
}
