import Foundation

/// The 18 outdoor-activity types — the soul of the product. The Norwegian ``key``
/// is the stable identifier. This is a PURE type (no SwiftUI): the localized
/// display label and SF Symbol / tint visuals live in `CoreDesignSystem`
/// (`ActivityKindVisuals`).
///
/// Mirrors `domain.ActivityKindId` in the Android app — keep the keys identical
/// so the two clients and the API agree.
public enum ActivityKindId: String, CaseIterable, Sendable, Codable {
    case mountain
    case park
    case beach
    case forest
    case hiking
    case kayaking
    case biking
    case cabin
    case parking
    case camping
    case swimming
    case diving
    case viewpoint
    case restaurant
    case cafe
    case accommodation
    case fishing
    case skiing

    /// The stable Norwegian key used by markers, search and the API.
    public var key: String {
        switch self {
        case .mountain: "Fjell"
        case .park: "Park"
        case .beach: "Strand"
        case .forest: "Skog"
        case .hiking: "Vandring"
        case .kayaking: "Kajakk"
        case .biking: "Sykkel"
        case .cabin: "Hytte"
        case .parking: "Parkering"
        case .camping: "Camping"
        case .swimming: "Badeplass"
        case .diving: "Dykking"
        case .viewpoint: "Utkikkspunkt"
        case .restaurant: "Restaurant"
        case .cafe: "Kafé"
        case .accommodation: "Overnatting"
        case .fishing: "Fiskeplass"
        case .skiing: "Ski"
        }
    }

    public static func fromKey(_ key: String) -> ActivityKindId? {
        allCases.first { $0.key == key }
    }
}
