import Foundation
import CoreModel

/// Nasjonal Turbase (ut.no / DNT) markers, served by the Turbo backend proxy
/// (`/api/places/ntb`). The proxy holds the api key and normalises the data, so
/// this client just does HTTP + DTO→domain mapping. Failures degrade to
/// empty/`nil` so a flaky source never breaks the map. Mirrors Android's
/// `core.data.NasjonalTurbaseRepository`.
public protocol NasjonalTurbaseRepository: Sendable {
    /// Cabins, places and trip markers within `bounds`.
    func pois(in bounds: GeoBounds) async -> [NtbPoi]
    /// A trip's full route (polyline + metadata), or `nil`.
    func route(id: String) async -> NtbRoute?
}

/// Real client against the public Places proxy (`/api/places/ntb` is open, no auth).
public struct HttpNasjonalTurbaseRepository: NasjonalTurbaseRepository {
    private let baseURL: URL
    private let session: URLSession

    public init(
        baseURL: URL = URL(string: "https://kart-api.sandring.no/api/places/ntb")!,
        session: URLSession = .shared
    ) {
        self.baseURL = baseURL
        self.session = session
    }

    public func pois(in bounds: GeoBounds) async -> [NtbPoi] {
        var components = URLComponents(
            url: baseURL.appendingPathComponent("pois"),
            resolvingAgainstBaseURL: false
        )
        components?.queryItems = [
            URLQueryItem(name: "minLat", value: String(bounds.south)),
            URLQueryItem(name: "minLon", value: String(bounds.west)),
            URLQueryItem(name: "maxLat", value: String(bounds.north)),
            URLQueryItem(name: "maxLon", value: String(bounds.east)),
        ]
        guard let url = components?.url else { return [] }
        do {
            let (data, response) = try await session.data(from: url)
            guard (response as? HTTPURLResponse).map({ (200..<300).contains($0.statusCode) }) ?? true else { return [] }
            let decoded = try JSONDecoder().decode(PoisResponseDTO.self, from: data)
            return decoded.pois.compactMap { $0.toDomain() }
        } catch {
            return []
        }
    }

    public func route(id: String) async -> NtbRoute? {
        let url = baseURL.appendingPathComponent("route").appendingPathComponent(id)
        do {
            let (data, response) = try await session.data(from: url)
            guard (response as? HTTPURLResponse).map({ (200..<300).contains($0.statusCode) }) ?? true else { return nil }
            return try JSONDecoder().decode(NtbRouteDTO.self, from: data).toDomain()
        } catch {
            return nil
        }
    }
}

// MARK: - Proxy DTOs (already normalised by the proxy into this shape)

private struct PoisResponseDTO: Decodable {
    let pois: [NtbPoiDTO]
}

private struct NtbPoiDTO: Decodable {
    let id: String
    let type: String
    let lat: Double?
    let lng: Double?
    let title: String
    let summary: String?
    let imageUrl: String?
    let utUrl: String?

    func toDomain() -> NtbPoi? {
        guard let lat, let lng else { return nil }
        let kind: NtbPoiType = switch type {
        case "cabin": .cabin
        case "trip": .trip
        default: .place
        }
        return NtbPoi(
            id: id, type: kind, title: title, position: LatLng(lat: lat, lng: lng),
            summary: summary, imageUrl: imageUrl, utUrl: utUrl
        )
    }
}

private struct NtbRouteDTO: Decodable {
    let id: String
    let title: String
    /// GeoJSON order: `[lng, lat]` pairs.
    let points: [[Double]]
    let description: String?
    let distanceMeters: Double?
    let grade: String?
    let imageUrl: String?
    let utUrl: String?

    func toDomain() -> NtbRoute {
        NtbRoute(
            id: id,
            title: title,
            points: points.compactMap { $0.count >= 2 ? LatLng(lat: $0[1], lng: $0[0]) : nil },
            description: description,
            distanceMeters: distanceMeters,
            grade: grade,
            imageUrl: imageUrl,
            utUrl: utUrl
        )
    }
}

/// Offline stand-in for previews / tests (and a no-network fallback): returns a
/// couple of demo POIs near the viewport centre so the overlay, info sheet and
/// route reveal can be driven without a backend.
public struct InMemoryNasjonalTurbaseRepository: NasjonalTurbaseRepository {
    public init() {}

    public func pois(in bounds: GeoBounds) async -> [NtbPoi] {
        let cLat = (bounds.south + bounds.north) / 2
        let cLng = (bounds.west + bounds.east) / 2
        return [
            NtbPoi(
                id: "demo-cabin", type: .cabin, title: "Demo cabin (DNT)",
                position: LatLng(lat: cLat + 0.01, lng: cLng + 0.01),
                summary: "A self-served DNT cabin. Sample data.",
                utUrl: "https://ut.no/hytte/demo"
            ),
            NtbPoi(
                id: "demo-trip", type: .trip, title: "Demo trip (UT.no)",
                position: LatLng(lat: cLat - 0.01, lng: cLng - 0.01),
                summary: "A marked trip suggestion. Sample data.",
                utUrl: "https://ut.no/turforslag/demo"
            ),
        ]
    }

    public func route(id: String) async -> NtbRoute? {
        guard id == "demo-trip" else { return nil }
        return NtbRoute(
            id: id, title: "Demo trip (UT.no)",
            points: [
                LatLng(lat: 59.90, lng: 10.70),
                LatLng(lat: 59.905, lng: 10.715),
                LatLng(lat: 59.912, lng: 10.72),
                LatLng(lat: 59.92, lng: 10.74),
            ],
            description: "A short demo route used to drive the reveal animation offline.",
            distanceMeters: 4200, grade: "Middels", utUrl: "https://ut.no/turforslag/demo"
        )
    }
}
