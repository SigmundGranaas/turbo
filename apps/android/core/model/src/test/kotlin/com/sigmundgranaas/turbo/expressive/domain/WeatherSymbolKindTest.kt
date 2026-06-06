package com.sigmundgranaas.turbo.expressive.domain

import org.junit.Assert.assertEquals
import org.junit.Test

class WeatherSymbolKindTest {

    @Test
    fun `classifies common codes, ignoring day-night suffix`() {
        assertEquals(WeatherKind.Clear, classifyWeatherSymbol("clearsky_day"))
        assertEquals(WeatherKind.Clear, classifyWeatherSymbol("fair_night"))
        assertEquals(WeatherKind.PartlyCloudy, classifyWeatherSymbol("partlycloudy_polartwilight"))
        assertEquals(WeatherKind.Cloudy, classifyWeatherSymbol("cloudy"))
        assertEquals(WeatherKind.Rain, classifyWeatherSymbol("lightrainshowers_day"))
        assertEquals(WeatherKind.Snow, classifyWeatherSymbol("heavysnow"))
        assertEquals(WeatherKind.Sleet, classifyWeatherSymbol("lightsleet"))
        assertEquals(WeatherKind.Fog, classifyWeatherSymbol("fog"))
    }

    @Test
    fun `combined hazards resolve to the more salient family`() {
        assertEquals(WeatherKind.Thunder, classifyWeatherSymbol("rainshowersandthunder_day"))
        assertEquals(WeatherKind.Snow, classifyWeatherSymbol("snowshowers_night"))
        assertEquals(WeatherKind.Unknown, classifyWeatherSymbol(null))
        assertEquals(WeatherKind.Unknown, classifyWeatherSymbol("nonsense"))
    }
}
