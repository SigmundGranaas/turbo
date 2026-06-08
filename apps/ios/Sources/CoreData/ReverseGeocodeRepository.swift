import Foundation
import CoreModel

/// Turns a coordinate into a human description (nearest Kartverket place name +
/// qualifier). Mirrors `core.data.ReverseGeocodeRepository`. Parsing is factored
/// out for unit testing.
public protocol ReverseGeocodeRepository: Sendable {
    func describe(_ point: LatLng) async -> LocationDescription?
}

public struct KartverketReverseGeocodeRepository: ReverseGeocodeRepository {
    private let session: URLSession
    private static let userAgent = "Turbo/0.1 github.com/SigmundGranaas/turbo"

    public init(session: URLSession = .shared) { self.session = session }

    public func describe(_ point: LatLng) async -> LocationDescription? {
        var components = URLComponents(string: "https://ws.geonorge.no/stedsnavn/v1/punkt")!
        components.queryItems = [
            URLQueryItem(name: "nord", value: String(format: "%.5f", point.lat)),
            URLQueryItem(name: "ost", value: String(format: "%.5f", point.lng)),
            URLQueryItem(name: "koordsys", value: "4258"),
            URLQueryItem(name: "radius", value: "1000"),
            URLQueryItem(name: "treffPerSide", value: "25"),
            URLQueryItem(name: "navnestatus", value: "hovednavn"),
        ]
        guard let url = components.url else { return nil }
        var request = URLRequest(url: url)
        request.setValue(KartverketReverseGeocodeRepository.userAgent, forHTTPHeaderField: "User-Agent")
        guard let (data, _) = try? await session.data(for: request) else { return nil }
        return Self.describe(data)
    }

    // MARK: - Parsing (testable)

    struct NearbyName: Equatable { let name: String; let type: String; let distanceM: Double }

    static func describe(_ data: Data) -> LocationDescription? {
        guard let nearest = nearest(parse(data)) else { return nil }
        return LocationDescription(title: nearest.name, qualifier: qualifier(for: nearest.type), secondary: nearest.type)
    }

    static func parse(_ data: Data) -> [NearbyName] {
        guard let dto = try? JSONDecoder().decode(Response.self, from: data) else { return [] }
        return dto.navn.compactMap { place in
            guard let name = place.skrivemate, !name.isEmpty else { return nil }
            return NearbyName(name: name, type: place.navneobjekttype ?? "", distanceM: place.meterFraPunkt ?? .greatestFiniteMagnitude)
        }
    }

    static func nearest(_ candidates: [NearbyName]) -> NearbyName? {
        candidates.min { $0.distanceM < $1.distanceM }
    }

    static func qualifier(for type: String) -> PlaceQualifier {
        let t = type.lowercased()
        switch true {
        case t.contains("fjell") || t.contains("tind") || t.contains("topp") || t.contains("nut") || t.contains("haug"): return .on
        case t.contains("by") || t.contains("tettsted") || t.contains("bygd") || t.contains("grend") || t.contains("sted"): return .in
        case t.contains("vatn") || t.contains("vann") || t.contains("elv") || t.contains("tjern") || t.contains("sjø"): return .at
        default: return .near
        }
    }

    private struct Response: Decodable { let navn: [Place] }
    private struct Place: Decodable {
        let skrivemate: String?
        let navneobjekttype: String?
        let meterFraPunkt: Double?
        enum CodingKeys: String, CodingKey {
            case skrivemate = "skrivemåte"
            case navneobjekttype
            case meterFraPunkt
        }
    }
}

/// A scripted reverse-geocoder for tests / hermetic runs.
public struct InMemoryReverseGeocodeRepository: ReverseGeocodeRepository {
    private let result: LocationDescription?
    public init(result: LocationDescription? = LocationDescription(title: "Lyngen", qualifier: .in, secondary: "kommune")) {
        self.result = result
    }
    public func describe(_ point: LatLng) async -> LocationDescription? { result }
}
