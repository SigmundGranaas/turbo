import Foundation

/// A place-search result (e.g. from Kartverket stedsnavn). Mirrors `domain.SearchHit`.
public struct SearchHit: Identifiable, Hashable, Sendable {
    public let id: String
    public let name: String
    public let description: String
    public let position: LatLng
    /// Activity-ish kind for the leading glyph (summit, fishing…), when known.
    public let kind: ActivityKindId?

    public init(name: String, description: String, position: LatLng, kind: ActivityKindId? = nil) {
        self.id = "\(name)|\(position.lat),\(position.lng)"
        self.name = name
        self.description = description
        self.position = position
        self.kind = kind
    }
}

/// A place the user has previously picked from search, surfaced when the search
/// field is empty. Most-recent-first. Mirrors `domain.RecentSearch`.
public struct RecentSearch: Identifiable, Hashable, Sendable {
    public let name: String
    public let sub: String
    public let lat: Double
    public let lng: Double

    public var id: String { "\(name)|\(lat),\(lng)" }
    public var position: LatLng { LatLng(lat: lat, lng: lng) }

    public init(name: String, sub: String, lat: Double, lng: Double) {
        self.name = name
        self.sub = sub
        self.lat = lat
        self.lng = lng
    }
}
