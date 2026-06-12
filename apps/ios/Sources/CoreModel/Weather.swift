import Foundation

/// A weather symbol, mapped to an SF Symbol. Mirrors `domain.WeatherSymbolKind`.
public enum WeatherSymbolKind: String, Sendable, Codable, CaseIterable {
    case clear, partlyCloudy, cloudy, fog, rain, sleet, snow, thunder, wind

    public var sfSymbol: String {
        switch self {
        case .clear: "sun.max.fill"
        case .partlyCloudy: "cloud.sun.fill"
        case .cloudy: "cloud.fill"
        case .fog: "cloud.fog.fill"
        case .rain: "cloud.rain.fill"
        case .sleet: "cloud.sleet.fill"
        case .snow: "cloud.snow.fill"
        case .thunder: "cloud.bolt.rain.fill"
        case .wind: "wind"
        }
    }
}

public struct HourForecast: Hashable, Sendable {
    public let label: String        // "14", "Now"
    public let temperatureC: Double
    public let symbol: WeatherSymbolKind
    public init(label: String, temperatureC: Double, symbol: WeatherSymbolKind) {
        self.label = label; self.temperatureC = temperatureC; self.symbol = symbol
    }
}

public struct DayForecast: Hashable, Sendable, Identifiable {
    public let weekday: String
    public let lowC: Double
    public let highC: Double
    public let symbol: WeatherSymbolKind
    public var id: String { weekday }
    public init(weekday: String, lowC: Double, highC: Double, symbol: WeatherSymbolKind) {
        self.weekday = weekday; self.lowC = lowC; self.highC = highC; self.symbol = symbol
    }
}

/// A forecast for a place. Mirrors `domain.WeatherSummary`.
public struct WeatherSummary: Hashable, Sendable {
    public let placeName: String
    public let temperatureC: Double
    public let symbol: WeatherSymbolKind
    public let summary: String
    public let hourly: [HourForecast]
    public let daily: [DayForecast]
    /// Wind at the location, when available (real met.no instant details).
    public let windSpeedMps: Double?
    public let windFromDegrees: Double?

    public init(placeName: String, temperatureC: Double, symbol: WeatherSymbolKind, summary: String,
                hourly: [HourForecast], daily: [DayForecast],
                windSpeedMps: Double? = nil, windFromDegrees: Double? = nil) {
        self.placeName = placeName
        self.temperatureC = temperatureC
        self.symbol = symbol
        self.summary = summary
        self.hourly = hourly
        self.daily = daily
        self.windSpeedMps = windSpeedMps
        self.windFromDegrees = windFromDegrees
    }

    /// `"−3°"` — rounded, with a real minus sign.
    public static func formatTemperature(_ celsius: Double) -> String {
        let rounded = Int(celsius.rounded())
        return rounded < 0 ? "−\(abs(rounded))°" : "\(rounded)°"
    }
}
