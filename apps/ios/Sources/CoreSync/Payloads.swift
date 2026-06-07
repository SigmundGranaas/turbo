import Foundation
import CoreModel

/// The wire form of a ``SavedPath`` for sync (geometry encoded inline).
public struct PathPayload: Codable, Sendable, Equatable {
    public let name: String
    public let activityKey: String?
    public let source: String
    public let points: [LatLng]
    public let elevations: [Double]?

    public init(_ path: SavedPath) {
        name = path.name
        activityKey = path.activityKind?.key
        source = path.path.source.rawValue
        points = path.path.points
        elevations = path.path.elevations
    }

    public func savedPath(id: String) -> SavedPath {
        SavedPath(
            id: id,
            name: name,
            path: GeoPath(points: points, source: GeoPathSource(rawValue: source) ?? .saved, elevations: elevations),
            activityKind: activityKey.flatMap(ActivityKindId.fromKey)
        )
    }
}

/// The wire form of a ``MapCollection`` for sync.
public struct CollectionPayload: Codable, Sendable, Equatable {
    public let name: String
    public let icon: String?
    public let itemCount: Int

    public init(_ collection: MapCollection) {
        name = collection.name
        icon = collection.icon
        itemCount = collection.itemCount
    }

    public func collection(id: String) -> MapCollection {
        MapCollection(id: id, name: name, icon: icon, itemCount: itemCount)
    }
}
