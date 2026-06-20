import Foundation

/// What kind of Nasjonal Turbase (ut.no / DNT) object a marker represents.
/// Mirrors `domain.NtbPoiType` (Android).
public enum NtbPoiType: String, Sendable, Codable {
    case cabin
    case trip
    case place
}

/// A lightweight Nasjonal Turbase marker (cabin / trip / place) as served by the
/// backend proxy (`/api/places/ntb/pois`). The proxy already normalised it, so
/// this is a thin value type. A `.trip` additionally has a route polyline fetched
/// lazily via ``NasjonalTurbaseRepository/route(id:)``. Mirrors `domain.NtbPoi`.
public struct NtbPoi: Identifiable, Hashable, Sendable, Codable {
    public let id: String
    public let type: NtbPoiType
    public let title: String
    public let position: LatLng
    public let summary: String?
    public let imageUrl: String?
    /// Best link back to ut.no (proxy-provided), or `nil`.
    public let utUrl: String?

    public init(
        id: String,
        type: NtbPoiType,
        title: String,
        position: LatLng,
        summary: String? = nil,
        imageUrl: String? = nil,
        utUrl: String? = nil
    ) {
        self.id = id
        self.type = type
        self.title = title
        self.position = position
        self.summary = summary
        self.imageUrl = imageUrl
        self.utUrl = utUrl
    }

    public var hasRoute: Bool { type == .trip }
}

/// A trip's full detail: the route polyline to reveal plus sheet metadata.
/// Mirrors `domain.NtbRoute`.
public struct NtbRoute: Identifiable, Hashable, Sendable, Codable {
    public let id: String
    public let title: String
    public let points: [LatLng]
    public let description: String?
    public let distanceMeters: Double?
    public let grade: String?
    public let imageUrl: String?
    public let utUrl: String?

    public init(
        id: String,
        title: String,
        points: [LatLng],
        description: String? = nil,
        distanceMeters: Double? = nil,
        grade: String? = nil,
        imageUrl: String? = nil,
        utUrl: String? = nil
    ) {
        self.id = id
        self.title = title
        self.points = points
        self.description = description
        self.distanceMeters = distanceMeters
        self.grade = grade
        self.imageUrl = imageUrl
        self.utUrl = utUrl
    }

    public var hasGeometry: Bool { points.count >= 2 }
}
