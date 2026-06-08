import Foundation
import CoreModel

/// Real weather from MET Norway's free Locationforecast API (no key; requires a
/// `User-Agent`). Mirrors the weather side of Android's `HttpConditionsRepository`.
/// Parsing is factored into ``parse(_:placeName:)`` so it's unit-testable offline.
public struct MetNoWeatherProvider: WeatherProvider {
    private let session: URLSession
    static let userAgent = "Turbo/0.1 github.com/SigmundGranaas/turbo"

    public init(session: URLSession = .shared) { self.session = session }

    public func forecast(at position: LatLng, placeName: String) async -> WeatherSummary? {
        var components = URLComponents(string: "https://api.met.no/weatherapi/locationforecast/2.0/compact")!
        components.queryItems = [
            URLQueryItem(name: "lat", value: String(format: "%.4f", position.lat)),
            URLQueryItem(name: "lon", value: String(format: "%.4f", position.lng)),
        ]
        guard let url = components.url else { return nil }
        var request = URLRequest(url: url)
        request.setValue(MetNoWeatherProvider.userAgent, forHTTPHeaderField: "User-Agent")
        do {
            let (data, _) = try await session.data(for: request)
            return Self.parse(data, placeName: placeName)
        } catch {
            return nil
        }
    }

    // MARK: - Parsing (network-free, testable)

    static func parse(_ data: Data, placeName: String) -> WeatherSummary? {
        guard let response = try? JSONDecoder().decode(Response.self, from: data),
              let series = response.properties?.timeseries, !series.isEmpty,
              let nowTemp = series.first?.data?.instant?.details?.airTemperature else { return nil }

        let nowSymbol = symbol(series.first?.data?.next1Hours?.summary?.symbolCode)

        let hourly: [HourForecast] = series.prefix(12).enumerated().compactMap { index, point in
            guard let temp = point.data?.instant?.details?.airTemperature else { return nil }
            return HourForecast(
                label: index == 0 ? "Now" : hourLabel(point.time),
                temperatureC: temp,
                symbol: symbol(point.data?.next1Hours?.summary?.symbolCode)
            )
        }

        // Group remaining series into days for the multi-day list.
        var byDay: [(day: String, temps: [Double], symbol: WeatherSymbolKind)] = []
        var seen: [String: Int] = [:]
        for point in series {
            guard let time = point.time, let temp = point.data?.instant?.details?.airTemperature else { continue }
            let day = String(time.prefix(10))
            if let i = seen[day] {
                byDay[i].temps.append(temp)
            } else {
                seen[day] = byDay.count
                byDay.append((day, [temp], symbol(point.data?.next6Hours?.summary?.symbolCode
                                                  ?? point.data?.next1Hours?.summary?.symbolCode)))
            }
        }
        let daily: [DayForecast] = byDay.prefix(7).enumerated().map { index, d in
            DayForecast(weekday: index == 0 ? "Today" : weekday(d.day),
                        lowC: d.temps.min() ?? 0, highC: d.temps.max() ?? 0, symbol: d.symbol)
        }

        return WeatherSummary(
            placeName: placeName, temperatureC: nowTemp, symbol: nowSymbol,
            summary: phrase(nowSymbol), hourly: hourly, daily: daily
        )
    }

    /// MET symbol codes (e.g. `partlycloudy_day`, `lightsnow`) → our symbol kinds.
    static func symbol(_ code: String?) -> WeatherSymbolKind {
        guard let c = code?.lowercased() else { return .cloudy }
        switch true {
        case c.contains("thunder"): return .thunder
        case c.contains("sleet"): return .sleet
        case c.contains("snow"): return .snow
        case c.contains("rain"): return .rain
        case c.contains("fog"): return .fog
        case c.contains("partlycloudy"): return .partlyCloudy
        case c.contains("cloud"): return .cloudy
        case c.contains("clear") || c.contains("fair"): return .clear
        default: return .cloudy
        }
    }

    private static func phrase(_ s: WeatherSymbolKind) -> String {
        switch s {
        case .clear: "Clear"
        case .partlyCloudy: "Partly cloudy"
        case .cloudy: "Cloudy"
        case .fog: "Fog"
        case .rain: "Rain"
        case .sleet: "Sleet"
        case .snow: "Snow"
        case .thunder: "Thunderstorms"
        case .wind: "Windy"
        }
    }

    private static func hourLabel(_ time: String?) -> String {
        // "2026-06-08T14:00:00Z" → "14"
        guard let time, time.count >= 13 else { return "" }
        return String(time[time.index(time.startIndex, offsetBy: 11)..<time.index(time.startIndex, offsetBy: 13)])
    }

    private static func weekday(_ day: String) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        guard let date = formatter.date(from: day) else { return day }
        let out = DateFormatter()
        out.dateFormat = "EEE"
        return out.string(from: date)
    }

    // MARK: - Wire types

    private struct Response: Decodable { let properties: Properties? }
    private struct Properties: Decodable { let timeseries: [Series]? }
    private struct Series: Decodable { let time: String?; let data: PointData? }
    private struct PointData: Decodable {
        let instant: Instant?
        let next1Hours: Hours?
        let next6Hours: Hours?
        enum CodingKeys: String, CodingKey {
            case instant
            case next1Hours = "next_1_hours"
            case next6Hours = "next_6_hours"
        }
    }
    private struct Instant: Decodable { let details: Details? }
    private struct Details: Decodable {
        let airTemperature: Double?
        enum CodingKeys: String, CodingKey { case airTemperature = "air_temperature" }
    }
    private struct Hours: Decodable { let summary: Summary? }
    private struct Summary: Decodable {
        let symbolCode: String?
        enum CodingKeys: String, CodingKey { case symbolCode = "symbol_code" }
    }
}
