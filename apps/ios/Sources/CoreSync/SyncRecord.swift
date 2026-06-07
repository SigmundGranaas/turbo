import Foundation
import CoreModel

/// A versioned, syncable record. The `updatedAt` epoch-ms drives last-write-wins;
/// `deleted` carries a tombstone so removals propagate. Mirrors the shape of the
/// Android sync DTOs (`core.sync.SyncDtos`).
public struct SyncRecord<Payload: Codable & Sendable>: Codable, Sendable, Equatable where Payload: Equatable {
    public let id: String
    public let updatedAt: Int64
    public let deleted: Bool
    public let payload: Payload?

    public init(id: String, updatedAt: Int64, deleted: Bool = false, payload: Payload?) {
        self.id = id
        self.updatedAt = updatedAt
        self.deleted = deleted
        self.payload = payload
    }
}

/// The wire form of a ``Marker`` for sync.
public struct MarkerPayload: Codable, Sendable, Equatable {
    public let name: String
    public let kindKey: String
    public let lat: Double
    public let lng: Double
    public let colorArgb: Int64?
    public let notes: String?

    public init(_ marker: Marker) {
        name = marker.name
        kindKey = marker.kind.key
        lat = marker.position.lat
        lng = marker.position.lng
        colorArgb = marker.colorArgb
        notes = marker.notes
    }

    public func marker(id: String) -> Marker {
        Marker(
            id: id,
            name: name,
            kind: ActivityKindId.fromKey(kindKey) ?? .mountain,
            position: LatLng(lat: lat, lng: lng),
            colorArgb: colorArgb,
            notes: notes
        )
    }
}
