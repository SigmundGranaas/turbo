import Foundation

/// Base-map tile sources (matches the layer-selector in the design).
/// Mirrors `domain.BaseLayer`.
public enum BaseLayer: String, CaseIterable, Sendable, Codable {
    case norgeskart
    case osm
    case satellite

    /// Stable tile-source id shared with the API and the offline manager.
    public var id: String {
        switch self {
        case .norgeskart: "topo"
        case .osm: "osm"
        case .satellite: "gs"
        }
    }

    public var title: String {
        switch self {
        case .norgeskart: "Norgeskart"
        case .osm: "OSM"
        case .satellite: "Satellite"
        }
    }
}

/// Toggleable data overlays painted over the map. Mirrors `domain.OverlayId`.
/// Only overlays with a real raster source ship — no dead toggles.
public enum OverlayId: String, CaseIterable, Sendable, Codable {
    case trails
    case avalanche

    public var title: String {
        switch self {
        case .trails: "Hiking trails"
        case .avalanche: "Avalanche danger"
        }
    }

    public var subtitle: String {
        switch self {
        case .trails: "Marked routes · Waymarked Trails"
        case .avalanche: "Steepness · NVE Bratthet"
        }
    }
}
