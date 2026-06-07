import Foundation
import CoreModel
import CoreCommon

/// Place search against Kartverket's stedsnavn API (ws.geonorge.no) — the
/// official Norwegian place-name service, mirroring the Android `SearchRepository`.
/// Recents are kept in a `ReactiveStore` (UserDefaults-persistable later).
///
/// The JSON decoding is factored into ``decodeHits(from:)`` so it can be unit
/// tested without hitting the network.
public final class KartverketSearchRepository: SearchRepository {
    private let recentStore: ReactiveStore<[RecentSearch]>
    private let session: URLSession
    private let endpoint = "https://ws.geonorge.no/stedsnavn/v1/navn"

    public init(recents: [RecentSearch] = [], session: URLSession = .shared) {
        self.recentStore = ReactiveStore(recents)
        self.session = session
    }

    public func search(_ query: String) async -> [SearchHit] {
        let q = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard q.count >= 2 else { return [] }
        var components = URLComponents(string: endpoint)!
        components.queryItems = [
            URLQueryItem(name: "sok", value: "\(q)*"),
            URLQueryItem(name: "fuzzy", value: "true"),
            URLQueryItem(name: "utkoordsys", value: "4258"),
            URLQueryItem(name: "treffPerSide", value: "12"),
            URLQueryItem(name: "side", value: "1"),
        ]
        guard let url = components.url else { return [] }
        do {
            let (data, _) = try await session.data(from: url)
            return try Self.decodeHits(from: data)
        } catch {
            return []
        }
    }

    public func recents() async -> AsyncStream<[RecentSearch]> { await recentStore.stream() }

    public func addRecent(_ recent: RecentSearch) async {
        await recentStore.update { list in
            ([recent] + list.filter { $0.id != recent.id }).prefix(8).map { $0 }
        }
    }

    // MARK: - Decoding (testable, network-free)

    static func decodeHits(from data: Data) throws -> [SearchHit] {
        let response = try JSONDecoder().decode(Response.self, from: data)
        return response.navn.map { place in
            let kommune = place.kommuner?.first?.kommunenavn
            let description = [place.navneobjekttype, kommune].compactMap { $0 }.joined(separator: " · ")
            return SearchHit(
                name: place.skrivemate,
                description: description,
                position: LatLng(lat: place.representasjonspunkt.nord, lng: place.representasjonspunkt.ost),
                kind: place.navneobjekttype.flatMap(kind(forPlaceType:))
            )
        }
    }

    /// Map a Kartverket `navneobjekttype` to the nearest activity kind (for the glyph).
    private static func kind(forPlaceType type: String) -> ActivityKindId? {
        switch type.lowercased() {
        case let t where t.contains("fjell") || t.contains("tind") || t.contains("topp"): .mountain
        case let t where t.contains("vatn") || t.contains("vann") || t.contains("elv") || t.contains("tjern"): .fishing
        case let t where t.contains("skog"): .forest
        case let t where t.contains("strand"): .beach
        case let t where t.contains("hytte") || t.contains("koie"): .cabin
        case let t where t.contains("øy") || t.contains("nes"): .viewpoint
        default: nil
        }
    }

    // MARK: - Wire types (Norwegian field names → ASCII via CodingKeys)

    private struct Response: Decodable { let navn: [Place] }

    private struct Place: Decodable {
        let skrivemate: String
        let navneobjekttype: String?
        let kommuner: [Kommune]?
        let representasjonspunkt: Point

        enum CodingKeys: String, CodingKey {
            case skrivemate = "skrivemåte"
            case navneobjekttype
            case kommuner
            case representasjonspunkt
        }
    }

    private struct Kommune: Decodable { let kommunenavn: String? }

    private struct Point: Decodable {
        let ost: Double
        let nord: Double
        enum CodingKeys: String, CodingKey {
            case ost = "øst"
            case nord
        }
    }
}
