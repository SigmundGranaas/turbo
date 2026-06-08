import Foundation
import CoreModel

/// Current marine conditions for a coastal point. Mirrors Flutter `YrOceanService`
/// (current-point slice). Returns nil outside MET's Nordic-seas footprint (the
/// endpoint answers 404/422 there) so the caller can hide marine UI silently.
public protocol MarineProvider: Sendable {
    func conditions(at position: LatLng) async -> MarineConditions?
}

public struct MetNoMarineProvider: MarineProvider {
    private let session: URLSession
    private static let userAgent = "Turbo/0.1 github.com/SigmundGranaas/turbo"

    public init(session: URLSession = .shared) { self.session = session }

    public func conditions(at position: LatLng) async -> MarineConditions? {
        var components = URLComponents(string: "https://api.met.no/weatherapi/oceanforecast/2.0/complete")!
        components.queryItems = [
            URLQueryItem(name: "lat", value: String(format: "%.4f", position.lat)),
            URLQueryItem(name: "lon", value: String(format: "%.4f", position.lng)),
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

    static func parse(_ data: Data) -> MarineConditions? {
        guard let dto = try? JSONDecoder().decode(Response.self, from: data),
              let details = dto.properties.timeseries.first?.data.instant.details else { return nil }
        let conditions = MarineConditions(
            waveHeightM: details.seaSurfaceWaveHeight,
            seaTemperatureC: details.seaWaterTemperature,
            seaCurrentMs: details.seaWaterSpeed
        )
        return conditions.hasData ? conditions : nil
    }

    // MARK: - Wire types

    private struct Response: Decodable { let properties: Properties }
    private struct Properties: Decodable { let timeseries: [Entry] }
    private struct Entry: Decodable { let data: PointData }
    private struct PointData: Decodable { let instant: Instant }
    private struct Instant: Decodable { let details: Details }
    private struct Details: Decodable {
        let seaSurfaceWaveHeight: Double?
        let seaWaterTemperature: Double?
        let seaWaterSpeed: Double?
        enum CodingKeys: String, CodingKey {
            case seaSurfaceWaveHeight = "sea_surface_wave_height"
            case seaWaterTemperature = "sea_water_temperature"
            case seaWaterSpeed = "sea_water_speed"
        }
    }
}

public struct InMemoryMarineProvider: MarineProvider {
    private let result: MarineConditions?
    public init(result: MarineConditions? = nil) { self.result = result }
    public func conditions(at position: LatLng) async -> MarineConditions? { result }
}
