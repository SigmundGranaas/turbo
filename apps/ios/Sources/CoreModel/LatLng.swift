import Foundation

/// A simple WGS84 coordinate, independent of any map SDK type.
/// Mirrors `domain.LatLng` in the Android app.
public struct LatLng: Hashable, Sendable, Codable {
    public let lat: Double
    public let lng: Double

    public init(lat: Double, lng: Double) {
        self.lat = lat
        self.lng = lng
    }
}

/// A lat/lng bounding box (WGS84). Mirrors `domain.GeoBounds`.
public struct GeoBounds: Hashable, Sendable, Codable {
    public let south: Double
    public let west: Double
    public let north: Double
    public let east: Double

    public init(south: Double, west: Double, north: Double, east: Double) {
        self.south = south
        self.west = west
        self.north = north
        self.east = east
    }
}
