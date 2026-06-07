import Foundation

/// A user marker pinned on the map. Mirrors `domain.Marker`.
public struct Marker: Identifiable, Hashable, Sendable, Codable {
    public let id: String
    public let name: String
    public let kind: ActivityKindId
    public let position: LatLng
    /// Optional override tint (ARGB). `nil` → the kind's terracotta default.
    public let colorArgb: Int64?
    /// Free-text note the user attached to the pin.
    public let notes: String?

    public init(
        id: String,
        name: String,
        kind: ActivityKindId,
        position: LatLng,
        colorArgb: Int64? = nil,
        notes: String? = nil
    ) {
        self.id = id
        self.name = name
        self.kind = kind
        self.position = position
        self.colorArgb = colorArgb
        self.notes = notes
    }
}
