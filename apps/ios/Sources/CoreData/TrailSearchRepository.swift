import Foundation
import CoreModel

/// Search source backed by Geonorge's "Nasjonal turbase" WFS feed (friluftsruter2).
/// Mirrors Flutter `TrailSearchService`. Queries the WFS GetFeature endpoint with
/// a CQL `ILIKE` filter on the trail `navn` field and returns up to 10 matches
/// keyed by the trail's first vertex (good enough for "zoom to result").
public protocol TrailSearchRepository: Sendable {
    func searchTrails(_ query: String) async -> [SearchHit]
}

public struct GeonorgeTrailSearchRepository: TrailSearchRepository {
    private let session: URLSession
    private static let userAgent = "Turbo/0.1 github.com/SigmundGranaas/turbo"

    public init(session: URLSession = .shared) { self.session = session }

    public func searchTrails(_ query: String) async -> [SearchHit] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count >= 2 else { return [] }
        var components = URLComponents(string: "https://wfs.geonorge.no/skwms1/wfs.friluftsruter2")!
        let escaped = trimmed.replacingOccurrences(of: "'", with: "''")
        components.queryItems = [
            URLQueryItem(name: "SERVICE", value: "WFS"),
            URLQueryItem(name: "VERSION", value: "2.0.0"),
            URLQueryItem(name: "REQUEST", value: "GetFeature"),
            URLQueryItem(name: "TYPENAMES", value: "fotrute"),
            URLQueryItem(name: "OUTPUTFORMAT", value: "application/json"),
            URLQueryItem(name: "SRSNAME", value: "urn:ogc:def:crs:EPSG::4326"),
            URLQueryItem(name: "COUNT", value: "10"),
            URLQueryItem(name: "CQL_FILTER", value: "navn ILIKE '%\(escaped)%'"),
        ]
        guard let url = components.url else { return [] }
        var request = URLRequest(url: url)
        request.setValue(Self.userAgent, forHTTPHeaderField: "User-Agent")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        guard let (data, response) = try? await session.data(for: request),
              (response as? HTTPURLResponse)?.statusCode == 200 else { return [] }
        return Self.parse(data)
    }

    // MARK: - Parsing (testable, network-free)

    static func parse(_ data: Data) -> [SearchHit] {
        guard let dto = try? JSONDecoder().decode(FeatureCollection.self, from: data) else { return [] }
        return dto.features.compactMap { feature in
            let name = feature.properties.navn ?? ""
            guard !name.isEmpty, let position = firstPoint(feature.geometry) else { return nil }
            let description = [feature.properties.rutenummer, feature.properties.merkemetode]
                .compactMap { $0 }.joined(separator: " · ")
            return SearchHit(name: name, description: description, position: position, kind: .hiking)
        }
    }

    static func firstPoint(_ geometry: Geometry) -> LatLng? {
        let pair: [Double]?
        switch geometry.type {
        case "LineString": pair = geometry.lineCoordinates?.first
        case "MultiLineString": pair = geometry.multiLineCoordinates?.first?.first
        default: pair = nil
        }
        guard let p = pair, p.count >= 2 else { return nil }
        return LatLng(lat: p[1], lng: p[0])   // GeoJSON is [lon, lat]
    }

    // MARK: - Wire types

    struct FeatureCollection: Decodable { let features: [Feature] }
    struct Feature: Decodable { let properties: Properties; let geometry: Geometry }
    struct Properties: Decodable {
        let navn: String?
        let rutenummer: String?
        let merkemetode: String?
    }
    struct Geometry: Decodable {
        let type: String
        let lineCoordinates: [[Double]]?
        let multiLineCoordinates: [[[Double]]]?

        enum CodingKeys: String, CodingKey { case type, coordinates }
        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            type = try c.decode(String.self, forKey: .type)
            // coordinates shape depends on geometry type — decode opportunistically.
            lineCoordinates = try? c.decode([[Double]].self, forKey: .coordinates)
            multiLineCoordinates = try? c.decode([[[Double]]].self, forKey: .coordinates)
        }
    }
}

public struct InMemoryTrailSearchRepository: TrailSearchRepository {
    private let hits: [SearchHit]
    public init(hits: [SearchHit] = []) { self.hits = hits }
    public func searchTrails(_ query: String) async -> [SearchHit] {
        let q = query.lowercased()
        return hits.filter { $0.name.lowercased().contains(q) }
    }
}

/// Merges multiple search sources behind one `SearchRepository`. Place results
/// (Kartverket stedsnavn) come first, then trail matches; recents delegate to the
/// primary place repository. Mirrors Flutter `CompositeSearchService`.
public final class CompositeSearchRepository: SearchRepository {
    private let place: SearchRepository
    private let trails: TrailSearchRepository

    public init(place: SearchRepository, trails: TrailSearchRepository) {
        self.place = place
        self.trails = trails
    }

    public func search(_ query: String) async -> [SearchHit] {
        async let placeHits = place.search(query)
        async let trailHits = trails.searchTrails(query)
        let (places, paths) = await (placeHits, trailHits)
        // De-dup trails whose name already appears in place results.
        let names = Set(places.map { $0.name.lowercased() })
        let freshTrails = paths.filter { !names.contains($0.name.lowercased()) }
        return places + freshTrails
    }

    public func recents() async -> AsyncStream<[RecentSearch]> { await place.recents() }
    public func addRecent(_ recent: RecentSearch) async { await place.addRecent(recent) }
}
