package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.TideExtreme
import com.sigmundgranaas.turbo.expressive.domain.TideForecast
import com.sigmundgranaas.turbo.expressive.domain.TideKind
import io.ktor.client.HttpClient
import io.ktor.client.request.get
import io.ktor.client.request.header
import io.ktor.client.request.parameter
import io.ktor.client.statement.bodyAsText
import java.time.ZoneId
import java.time.ZonedDateTime
import java.time.format.DateTimeFormatter
import javax.inject.Inject

/**
 * Tide predictions (high/low extrema) from Kartverket's sehavniva endpoint. Coverage is
 * Norwegian coastal waters; elsewhere the API returns no rows → empty/failure, so the
 * caller hides the tide UI without an error state.
 */
interface TideRepository {
    suspend fun forPoint(point: LatLng): Outcome<TideForecast>
}

class KartverketTideRepository @Inject constructor(
    private val client: HttpClient,
) : TideRepository {

    override suspend fun forPoint(point: LatLng): Outcome<TideForecast> = runCatching {
        // The API wants Norwegian wall-clock time (yyyy-MM-ddTHH:mm), not the device zone.
        val now = ZonedDateTime.now(OSLO)
        val body = client
            .get("https://vannstand.kartverket.no/tideapi.php") {
                parameter("lat", "%.4f".format(point.lat))
                parameter("lon", "%.4f".format(point.lng))
                parameter("fromtime", now.minusHours(6).format(API_TIME))
                parameter("totime", now.plusDays(3).format(API_TIME))
                parameter("datatype", "tab") // high/low extrema only
                parameter("refcode", "cd")
                parameter("lang", "en")
                parameter("dst", "1")
                parameter("tide_request", "locationdata")
                header("User-Agent", USER_AGENT)
            }
            .bodyAsText()
        TideXml.parse(body) ?: error("No tide data")
    }.fold({ Outcome.Success(it) }, { Outcome.Failure(it) })

    private companion object {
        val OSLO: ZoneId = ZoneId.of("Europe/Oslo")
        val API_TIME: DateTimeFormatter = DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm")
        const val USER_AGENT = "turbo-expressive/0.1 github.com/SigmundGranaas/turbo"
    }
}

/**
 * Pure parser for the sehavniva XML, isolated from networking so the extraction +
 * flag mapping can be unit-tested. The response is a flat list of
 * `<waterlevel time=… value=… flag="high|low"/>` rows under `<location name=…>`.
 */
object TideXml {
    private val ELEMENT = Regex("""<waterlevel\b[^>]*?/?>""", RegexOption.IGNORE_CASE)
    private val TIME = Regex("""time="([^"]+)"""")
    private val VALUE = Regex("""value="([^"]+)"""")
    private val FLAG = Regex("""flag="([^"]+)"""")
    private val STATION = Regex("""<location\b[^>]*?name="([^"]+)"""", RegexOption.IGNORE_CASE)

    fun parse(body: String): TideForecast? {
        val extrema = ELEMENT.findAll(body).mapNotNull { m ->
            val el = m.value
            val timeRaw = TIME.find(el)?.groupValues?.get(1) ?: return@mapNotNull null
            val level = VALUE.find(el)?.groupValues?.get(1)?.toDoubleOrNull() ?: return@mapNotNull null
            val kind = kindOf(FLAG.find(el)?.groupValues?.get(1)) ?: return@mapNotNull null
            TideExtreme(timeIso = normalizeIso(timeRaw), levelCm = level, kind = kind)
        }.sortedBy { it.timeIso }.toList()
        if (extrema.isEmpty()) return null
        return TideForecast(stationName = STATION.find(body)?.groupValues?.get(1), extrema = extrema)
    }

    private fun kindOf(flag: String?): TideKind? = when (flag?.lowercase()) {
        "high", "hw" -> TideKind.High
        "low", "lw" -> TideKind.Low
        else -> null
    }

    /** sehavniva timestamps carry a zone offset (e.g. "…+01:00"); keep as-is for display
     *  ordering — string compare is monotone for same-offset ISO instants the API returns. */
    private fun normalizeIso(raw: String): String = raw.trim()
}
