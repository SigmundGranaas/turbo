import Foundation
import CoreModel

/// Weather for a coordinate. `nil` means "no forecast available" (the UI then
/// hides weather rather than inventing a value). The real implementation is
/// ``MetNoWeatherProvider``; this in-memory one is a deterministic test double.
public protocol WeatherProvider: Sendable {
    func forecast(at position: LatLng, placeName: String) async -> WeatherSummary?
}

public struct InMemoryWeatherProvider: WeatherProvider {
    public init() {}

    public func forecast(at position: LatLng, placeName: String) async -> WeatherSummary? {
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

/// Avalanche danger for a coordinate. `nil` means "no warning for this area"
/// (outside a forecast region, or none issued) — the UI then hides it rather than
/// showing a fake level. Real implementation is ``VarsomAvalancheProvider``.
public protocol AvalancheProvider: Sendable {
    func danger(at position: LatLng) async -> AvalancheInfo?
}

public struct InMemoryAvalancheProvider: AvalancheProvider {
    private let stub: AvalancheInfo?
    public init(stub: AvalancheInfo? = AvalancheInfo(region: "Lyngen", level: 3, headline: "Considerable — wind slabs on N–E aspects above 600 m.")) {
        self.stub = stub
    }
    public func danger(at position: LatLng) async -> AvalancheInfo? { stub }
}
