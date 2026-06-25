package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.TideExtreme
import com.sigmundgranaas.turbo.expressive.domain.TideForecast
import com.sigmundgranaas.turbo.expressive.domain.TideKind
import java.time.ZoneOffset
import java.time.ZonedDateTime
import java.time.format.DateTimeFormatter
import javax.inject.Inject

/**
 * Offline stand-in for [TideRepository] — Kartverket sehavniva can't be reached from the
 * emulator and covers Norway only, so the ocean section's tide table would otherwise be
 * empty. Fabricates a plausible semidiurnal cycle (a high/low roughly every 6 h 12 m) so
 * the UI is driveable anywhere. Selected in DEBUG via NetworkModule.
 */
class SyntheticTideRepository @Inject constructor() : TideRepository {

    override suspend fun forPoint(point: LatLng): Outcome<TideForecast> {
        val start = ZonedDateTime.now(ZoneOffset.UTC).withMinute(0).withSecond(0).withNano(0)
        val extrema = (0 until 6).map { i ->
            val high = i % 2 == 0
            TideExtreme(
                timeIso = start.plusMinutes(i * 372L).format(ISO), // 6 h 12 m apart
                levelCm = if (high) 86.0 else -14.0,
                kind = if (high) TideKind.High else TideKind.Low,
            )
        }
        return Outcome.Success(TideForecast(stationName = "Simulated harbour", extrema = extrema))
    }

    private companion object {
        val ISO: DateTimeFormatter = DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm:ss'Z'")
    }
}
