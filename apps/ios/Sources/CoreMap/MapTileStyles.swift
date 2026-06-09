import Foundation
import CoreModel

/// Raster XYZ tile sources for each base map, ported from `ui.map.MapStyles`
/// (Android). Norgeskart is the official Kartverket topo cache; OSM and an Esri
/// satellite layer are the alternates shown in the layer selector.
///
/// Templates use `{x}`/`{y}`/`{z}` placeholders (the `MKTileOverlay` convention).
/// Note the axis order differs per provider — Kartverket and Esri are `{z}/{y}/{x}`,
/// OSM is `{z}/{x}/{y}` — so keep these strings exact.
public enum MapTileStyles {

    public static func tileURLTemplate(for base: BaseLayer) -> String {
        switch base {
        case .norgeskart:
            "https://cache.kartverket.no/v1/wmts/1.0.0/topo/default/webmercator/{z}/{y}/{x}.png"
        case .osm:
            "https://tile.openstreetmap.org/{z}/{x}/{y}.png"
        case .satellite:
            "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}"
        }
    }

    public static func attribution(for base: BaseLayer) -> String {
        switch base {
        case .norgeskart: "© Kartverket"
        case .osm: "© OpenStreetMap contributors"
        case .satellite: "© Esri"
        }
    }

    /// Transparent XYZ overlay tiles for an ``OverlayId``, or `nil` when it has no
    /// shippable raster source (no dead toggles). Ported from `ui.map.MapStyles`.
    public static func overlayTemplate(for overlay: OverlayId) -> String? {
        switch overlay {
        case .trails:
            "https://tile.waymarkedtrails.org/hiking/{z}/{x}/{y}.png"
        case .avalanche:
            "https://gis3.nve.no/arcgis/rest/services/wmts/Bratthet_2024/MapServer/tile/{z}/{y}/{x}"
        }
    }

    /// The overlays that actually have a tile source today (drives the layer sheet).
    public static var renderableOverlays: [OverlayId] {
        OverlayId.allCases.filter { overlayTemplate(for: $0) != nil }
    }
}
