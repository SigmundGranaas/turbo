import Foundation

/// A user-defined grouping of map entities (markers, tracks). A lightweight
/// folder with a colour + optional icon and a membership count. Mirrors
/// `domain.MapCollection`.
public struct MapCollection: Identifiable, Hashable, Sendable {
    public let id: String
    public let name: String
    public let colorArgb: Int64?
    public let icon: String?
    public let itemCount: Int

    public init(id: String, name: String, colorArgb: Int64? = nil, icon: String? = nil, itemCount: Int = 0) {
        self.id = id
        self.name = name
        self.colorArgb = colorArgb
        self.icon = icon
        self.itemCount = itemCount
    }
}

/// The kinds of entity a collection can contain. Mirrors `domain.CollectionItemType`.
public enum CollectionItemType: String, Sendable, Codable {
    case marker, path
}
