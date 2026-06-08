import Foundation
import CoreModel

/// Sunrise / sunset for a place on a given day. Mirrors Flutter `YrSunriseService`
/// (single-day slice). Parsing is factored out for offline unit testing.
public protocol SunProvider: Sendable {
    func sun(at position: LatLng, date: Date) async -> SunTimes?
}

public struct MetNoSunProvider: SunProvider {
    private let session: URLSession
    private static let userAgent = "Turbo/0.1 github.com/SigmundGranaas/turbo"

    public init(session: URLSession = .shared) { self.session = session }

    public func sun(at position: LatLng, date: Date) async -> SunTimes? {
        var components = URLComponents(string: "https://api.met.no/weatherapi/sunrise/3.0/sun")!
        components.queryItems = [
            URLQueryItem(name: "lat", value: String(format: "%.4f", position.lat)),
            URLQueryItem(name: "lon", value: String(format: "%.4f", position.lng)),
            URLQueryItem(name: "date", value: Self.dateString(date)),
            URLQueryItem(name: "offset", value: Self.offsetString(TimeZone.current.secondsFromGMT(for: date))),
        ]
        guard let url = components.url else { return nil }
        var request = URLRequest(url: url)
        request.setValue(Self.userAgent, forHTTPHeaderField: "User-Agent")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        guard let (data, response) = try? await session.data(for: request),
              (response as? HTTPURLResponse)?.statusCode == 200 else { return nil }
        return Self.parse(data)
    }

    // MARK: - Parsing (testable, network-free)

    static func parse(_ data: Data) -> SunTimes? {
        guard let dto = try? JSONDecoder().decode(Response.self, from: data) else { return nil }
        let props = dto.properties
        let sunrise = props.sunrise?.time.flatMap(parseTime)
        let sunset = props.sunset?.time.flatMap(parseTime)
        // MET signals polar conditions by omitting both events; solar-noon
        // visibility distinguishes polar day (visible) from polar night.
        let noonVisible = props.solarnoon?.visible
        let polarDay = sunrise == nil && sunset == nil && noonVisible == true
        let polarNight = sunrise == nil && sunset == nil && noonVisible == false
        guard sunrise != nil || sunset != nil || polarDay || polarNight else { return nil }
        return SunTimes(sunrise: sunrise, sunset: sunset, polarDay: polarDay, polarNight: polarNight)
    }

    static func parseTime(_ iso: String) -> Date? {
        let withSeconds = ISO8601DateFormatter()
        withSeconds.formatOptions = [.withInternetDateTime]
        if let d = withSeconds.date(from: iso) { return d }
        // MET sometimes omits seconds ("...T02:11+02:00"); parse that too.
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd'T'HH:mmZZZZZ"
        return f.date(from: iso)
    }

    static func dateString(_ date: Date) -> String {
        let f = DateFormatter()
        f.calendar = Calendar(identifier: .gregorian)
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = .current
        f.dateFormat = "yyyy-MM-dd"
        return f.string(from: date)
    }

    static func offsetString(_ seconds: Int) -> String {
        let sign = seconds < 0 ? "-" : "+"
        let abs = Swift.abs(seconds)
        return String(format: "%@%02d:%02d", sign, abs / 3600, (abs % 3600) / 60)
    }

    // MARK: - Wire types

    private struct Response: Decodable { let properties: Properties }
    private struct Properties: Decodable {
        let sunrise: Event?
        let sunset: Event?
        let solarnoon: SolarNoon?
    }
    private struct Event: Decodable { let time: String? }
    private struct SolarNoon: Decodable { let visible: Bool? }
}

public struct InMemorySunProvider: SunProvider {
    private let result: SunTimes?
    public init(result: SunTimes? = nil) { self.result = result }
    public func sun(at position: LatLng, date: Date) async -> SunTimes? { result }
}
