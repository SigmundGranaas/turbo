import Foundation
import MapKit
import CoreModel

/// On-disk tile cache, keyed by base layer + `z/x/y`, shared across offline
/// regions (overlapping regions dedupe). Both ``DiskOfflineTileManager`` (writer)
/// and ``CachingTileOverlay`` (reader) go through this so they agree on layout.
public struct OfflineTileCache: Sendable {
    public let root: URL

    /// Default shared location under Caches.
    public static let `default` = OfflineTileCache(
        root: FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("turbo-offline", isDirectory: true)
    )

    public init(root: URL) { self.root = root }

    /// `<root>/tiles/<baseId>/<z>_<x>_<y>`
    public func fileURL(base: BaseLayer, z: Int, x: Int, y: Int) -> URL {
        root.appendingPathComponent("tiles/\(base.id)/\(z)_\(x)_\(y)")
    }

    func hasTile(base: BaseLayer, z: Int, x: Int, y: Int) -> Bool {
        FileManager.default.fileExists(atPath: fileURL(base: base, z: z, x: x, y: y).path)
    }
}

/// An `MKTileOverlay` that serves cached tiles from disk first and falls back to
/// the network — so downloaded regions render with no signal. This closes the
/// offline loop between ``DiskOfflineTileManager`` (downloads) and the live map.
public final class CachingTileOverlay: MKTileOverlay {
    private let base: BaseLayer
    private let cache: OfflineTileCache

    public init(base: BaseLayer, cache: OfflineTileCache = .default) {
        self.base = base
        self.cache = cache
        super.init(urlTemplate: MapTileStyles.tileURLTemplate(for: base))
        canReplaceMapContent = true
    }

    public override func url(forTilePath path: MKTileOverlayPath) -> URL {
        let cached = cache.fileURL(base: base, z: path.z, x: path.x, y: path.y)
        if FileManager.default.fileExists(atPath: cached.path) { return cached }
        return super.url(forTilePath: path)
    }
}
