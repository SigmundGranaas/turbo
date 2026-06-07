import Foundation
import CoreModel

/// A slippy-map (XYZ) tile coordinate.
public struct TileCoordinate: Hashable, Sendable {
    public let z: Int
    public let x: Int
    public let y: Int
    public init(z: Int, x: Int, y: Int) { self.z = z; self.x = x; self.y = y }
}

/// Web-Mercator tile math — the standard slippy-map projection used to turn a
/// lat/lng box at a zoom range into the set of tiles an offline region must cover.
public enum TileMath {

    /// The tile containing `(lat, lng)` at `zoom`.
    public static func tile(lat: Double, lng: Double, zoom: Int) -> TileCoordinate {
        let n = Double(1 << zoom)
        let x = Int(floor((lng + 180.0) / 360.0 * n))
        let latRad = lat * .pi / 180.0
        let y = Int(floor((1.0 - asinh(tan(latRad)) / .pi) / 2.0 * n))
        let maxIndex = (1 << zoom) - 1
        return TileCoordinate(z: zoom, x: x.clamped(0, maxIndex), y: y.clamped(0, maxIndex))
    }

    /// Every tile covering `bounds` across `minZoom...maxZoom`.
    public static func tiles(in bounds: GeoBounds, minZoom: Int, maxZoom: Int) -> [TileCoordinate] {
        var result: [TileCoordinate] = []
        for z in minZoom...max(minZoom, maxZoom) {
            let topLeft = tile(lat: bounds.north, lng: bounds.west, zoom: z)
            let bottomRight = tile(lat: bounds.south, lng: bounds.east, zoom: z)
            let xs = min(topLeft.x, bottomRight.x)...max(topLeft.x, bottomRight.x)
            let ys = min(topLeft.y, bottomRight.y)...max(topLeft.y, bottomRight.y)
            for x in xs { for y in ys { result.append(TileCoordinate(z: z, x: x, y: y)) } }
        }
        return result
    }

    /// Fill a tile-URL template (`{x}`/`{y}`/`{z}`) for a coordinate.
    public static func url(template: String, for tile: TileCoordinate) -> URL? {
        let filled = template
            .replacingOccurrences(of: "{z}", with: String(tile.z))
            .replacingOccurrences(of: "{x}", with: String(tile.x))
            .replacingOccurrences(of: "{y}", with: String(tile.y))
        return URL(string: filled)
    }
}

private extension Int {
    func clamped(_ lo: Int, _ hi: Int) -> Int { Swift.min(Swift.max(self, lo), hi) }
}
