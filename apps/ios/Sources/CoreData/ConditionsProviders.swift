import Foundation
import CoreModel

/// Weather for a coordinate. The real implementation uses WeatherKit (needs the
/// capability); this in-memory one returns a deterministic sample so the UI works
/// offline. Mirrors the weather side of Android's `ConditionsRepository`.
public protocol WeatherProvider: Sendable {
    func forecast(at position: LatLng, placeName: String) async -> WeatherSummary
}

public struct InMemoryWeatherProvider: WeatherProvider {
    public init() {}

    public func forecast(at position: LatLng, placeName: String) async -> WeatherSummary {
        let hours = (0..<12).map { i in
            HourForecast(label: i == 0 ? "Now" : "\((14 + i) % 24)",
                         temperatureC: -3 + Double(i) * 0.4,
                         symbol: i < 4 ? .snow : .partlyCloudy)
        }
        let days = ["Today", "Wed", "Thu", "Fri", "Sat", "Sun", "Mon"].enumerated().map { i, d in
            DayForecast(weekday: d, lowC: -8 + Double(i), highC: -1 + Double(i), symbol: i % 2 == 0 ? .snow : .cloudy)
        }
        return WeatherSummary(
            placeName: placeName, temperatureC: -3, symbol: .snow,
            summary: "Light snow. High winds above the treeline.",
            hourly: hours, daily: days
        )
    }
}

/// Avalanche danger for a coordinate. Real implementation hits the Varsom/NVE API;
/// in-memory returns a sample. Mirrors the avalanche side of `ConditionsRepository`.
public protocol AvalancheProvider: Sendable {
    func danger(at position: LatLng) async -> AvalancheInfo
}

public struct InMemoryAvalancheProvider: AvalancheProvider {
    public init() {}
    public func danger(at position: LatLng) async -> AvalancheInfo {
        AvalancheInfo(region: "Lyngen", level: 3, headline: "Considerable — wind slabs on N–E aspects above 600 m.")
    }
}
