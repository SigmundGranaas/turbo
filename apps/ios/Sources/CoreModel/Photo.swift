import Foundation

/// A geotagged photo stored locally. `markerId` links it to a marker when
/// attached (nil = standalone, shown only on the photo map). `uri` is a file URL
/// string the app can render. Mirrors `domain.Photo`.
public struct Photo: Identifiable, Hashable, Sendable, Codable {
    public let id: String
    public let markerId: String?
    public let lat: Double
    public let lng: Double
    public let uri: String
    public let capturedAtEpochMs: Int64

    public var position: LatLng { LatLng(lat: lat, lng: lng) }

    public init(id: String, markerId: String?, lat: Double, lng: Double, uri: String, capturedAtEpochMs: Int64) {
        self.id = id
        self.markerId = markerId
        self.lat = lat
        self.lng = lng
        self.uri = uri
        self.capturedAtEpochMs = capturedAtEpochMs
    }
}
