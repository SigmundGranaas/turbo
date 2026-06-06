package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.domain.AtmosphericPoint
import com.sigmundgranaas.turbo.expressive.domain.AvalancheNow
import com.sigmundgranaas.turbo.expressive.domain.AvalancheProblem
import com.sigmundgranaas.turbo.expressive.domain.Conditions
import com.sigmundgranaas.turbo.expressive.domain.MarineNow
import com.sigmundgranaas.turbo.expressive.domain.shouldShowAvalanche
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.WeatherForecast
import com.sigmundgranaas.turbo.expressive.domain.WeatherNow
import com.sigmundgranaas.turbo.expressive.domain.WeatherSummary
import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.header
import io.ktor.client.request.parameter
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import java.time.LocalDate
import javax.inject.Inject

/**
 * Live conditions for a coordinate: current weather from MET Norway's
 * locationforecast, and today's avalanche danger from NVE Varsom. Weather is
 * the primary signal; avalanche is best-effort and null when unavailable.
 */
interface ConditionsRepository {
    suspend fun forPoint(point: LatLng): Outcome<Conditions>

    /** The full hourly + daily forecast for the conditions detail sheet. */
    suspend fun forecast(point: LatLng): Outcome<WeatherForecast>
}

class HttpConditionsRepository @Inject constructor(
    private val client: HttpClient,
) : ConditionsRepository {

    override suspend fun forPoint(point: LatLng): Outcome<Conditions> {
        val weather = runCatching { fetchWeather(point) }.getOrNull()
        val raw = runCatching { fetchAvalanche(point) }.getOrNull()
        // Suppress low-confidence danger (L1, or L2 when warm) per the Flutter heuristic.
        val avalanche = raw?.takeIf { shouldShowAvalanche(it.dangerLevel, weather?.temperatureC) }
        // Marine is best-effort and null inland (oceanforecast 422s away from the coast).
        val marine = runCatching { fetchMarine(point) }.getOrNull()?.takeIf { it.hasData }
        return if (weather == null && avalanche == null && marine == null) {
            Outcome.Failure(IllegalStateException("No conditions available"))
        } else {
            Outcome.Success(Conditions(weather, avalanche, marine))
        }
    }

    override suspend fun forecast(point: LatLng): Outcome<WeatherForecast> =
        runCatching {
            val points = fetchTimeseries(point)
            if (points.isEmpty()) error("No forecast")
            WeatherForecast(points = points, days = WeatherSummary.dailySummaries(points))
        }.fold({ Outcome.Success(it) }, { Outcome.Failure(it) })

    private suspend fun fetchTimeseries(point: LatLng): List<AtmosphericPoint> {
        val res: MetResponse = client
            .get("https://api.met.no/weatherapi/locationforecast/2.0/compact") {
                parameter("lat", "%.4f".format(point.lat))
                parameter("lon", "%.4f".format(point.lng))
                header("User-Agent", USER_AGENT)
            }
            .body()
        return res.properties?.timeseries.orEmpty().mapNotNull { s ->
            val time = s.time ?: return@mapNotNull null
            val d = s.data?.instant?.details
            AtmosphericPoint(
                timeIso = time,
                temperatureC = d?.airTemperature,
                windSpeedMs = d?.windSpeed,
                windFromDeg = d?.windFromDirection,
                humidityPct = d?.relativeHumidity,
                cloudCoverPct = d?.cloudAreaFraction,
                uvIndex = d?.uvIndexClearSky,
                precipitation1hMm = s.data?.next1Hours?.details?.precipitationAmount,
                symbol1h = s.data?.next1Hours?.summary?.symbolCode,
            )
        }
    }

    private suspend fun fetchWeather(point: LatLng): WeatherNow? {
        val res: MetResponse = client
            .get("https://api.met.no/weatherapi/locationforecast/2.0/compact") {
                parameter("lat", "%.4f".format(point.lat))
                parameter("lon", "%.4f".format(point.lng))
                header("User-Agent", USER_AGENT)
            }
            .body()
        val first = res.properties?.timeseries?.firstOrNull() ?: return null
        val instant = first.data?.instant?.details
        return WeatherNow(
            temperatureC = instant?.airTemperature,
            windSpeedMs = instant?.windSpeed,
            windFromDeg = instant?.windFromDirection,
            precipitationMm = first.data?.next1Hours?.details?.precipitationAmount,
            symbolCode = first.data?.next1Hours?.summary?.symbolCode,
            humidityPct = instant?.relativeHumidity,
            cloudCoverPct = instant?.cloudAreaFraction,
            uvIndex = instant?.uvIndexClearSky,
        )
    }

    private suspend fun fetchMarine(point: LatLng): MarineNow? {
        val res: MetResponse = client
            .get("https://api.met.no/weatherapi/oceanforecast/2.0/complete") {
                parameter("lat", "%.4f".format(point.lat))
                parameter("lon", "%.4f".format(point.lng))
                header("User-Agent", USER_AGENT)
            }
            .body()
        val d = res.properties?.timeseries?.firstOrNull()?.data?.instant?.details ?: return null
        return MarineNow(
            waveHeightM = d.seaWaveHeight,
            waveFromDeg = d.seaWaveFromDirection,
            seaTemperatureC = d.seaWaterTemperature,
        )
    }

    private suspend fun fetchAvalanche(point: LatLng): AvalancheNow? {
        val today = LocalDate.now().toString()
        val warnings: List<VarsomWarning> = client
            .get(
                "https://api01.nve.no/hydrology/forecast/avalanche/v6.2.1/api/" +
                    "AvalancheWarningByCoordinates/Detail/${point.lat}/${point.lng}/1/$today/$today",
            )
            .body()
        val w = warnings.firstOrNull { (it.dangerLevel?.toIntOrNull() ?: 0) > 0 } ?: return null
        val level = w.dangerLevel?.toIntOrNull() ?: return null
        return AvalancheNow(
            dangerLevel = level,
            mainText = w.mainText.orEmpty(),
            region = w.regionName.orEmpty(),
            problems = w.avalancheProblems.orEmpty().map {
                AvalancheProblem(
                    type = it.problemTypeName?.takeIf(String::isNotBlank),
                    trigger = it.triggerName?.takeIf(String::isNotBlank),
                    distribution = it.distributionName?.takeIf(String::isNotBlank),
                    size = it.destructiveSizeName?.takeIf(String::isNotBlank),
                )
            },
        )
    }

    private companion object {
        const val USER_AGENT = "turbo-expressive/0.1 github.com/SigmundGranaas/turbo"
    }
}

@Serializable
private data class MetResponse(val properties: MetProperties? = null)

@Serializable
private data class MetProperties(val timeseries: List<MetSeries> = emptyList())

@Serializable
private data class MetSeries(val time: String? = null, val data: MetData? = null)

@Serializable
private data class MetData(
    val instant: MetInstant? = null,
    @SerialName("next_1_hours") val next1Hours: MetNextHours? = null,
)

@Serializable
private data class MetInstant(val details: MetInstantDetails? = null)

@Serializable
private data class MetInstantDetails(
    @SerialName("air_temperature") val airTemperature: Double? = null,
    @SerialName("wind_speed") val windSpeed: Double? = null,
    @SerialName("wind_from_direction") val windFromDirection: Double? = null,
    @SerialName("relative_humidity") val relativeHumidity: Double? = null,
    @SerialName("cloud_area_fraction") val cloudAreaFraction: Double? = null,
    @SerialName("ultraviolet_index_clear_sky") val uvIndexClearSky: Double? = null,
    @SerialName("sea_surface_wave_height") val seaWaveHeight: Double? = null,
    @SerialName("sea_surface_wave_from_direction") val seaWaveFromDirection: Double? = null,
    @SerialName("sea_water_temperature") val seaWaterTemperature: Double? = null,
)

@Serializable
private data class MetNextHours(
    val summary: MetSummary? = null,
    val details: MetNextDetails? = null,
)

@Serializable
private data class MetSummary(@SerialName("symbol_code") val symbolCode: String? = null)

@Serializable
private data class MetNextDetails(
    @SerialName("precipitation_amount") val precipitationAmount: Double? = null,
)

@Serializable
private data class VarsomWarning(
    @SerialName("DangerLevel") val dangerLevel: String? = null,
    @SerialName("MainText") val mainText: String? = null,
    @SerialName("RegionName") val regionName: String? = null,
    @SerialName("AvalancheProblems") val avalancheProblems: List<VarsomProblem> = emptyList(),
)

@Serializable
private data class VarsomProblem(
    @SerialName("AvalancheProblemTypeName") val problemTypeName: String? = null,
    @SerialName("AvalTriggerSimpleName") val triggerName: String? = null,
    @SerialName("AvalPropagationName") val distributionName: String? = null,
    @SerialName("DestructiveSizeExtName") val destructiveSizeName: String? = null,
)
