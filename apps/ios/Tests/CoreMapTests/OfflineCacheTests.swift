import Testing
import Foundation
import MapKit
import CoreModel
@testable import CoreMap

@Suite("Offline tile cache")
struct OfflineCacheTests {

    @Test("cache path is base-keyed by z/x/y under the root")
    func cachePath() {
        let root = URL(fileURLWithPath: "/tmp/turbo-x")
        let cache = OfflineTileCache(root: root)
        let url = cache.fileURL(base: .norgeskart, z: 7, x: 67, y: 34)
        #expect(url.path.hasSuffix("tiles/topo/7_67_34"))
    }

    @Test("a downloaded region's tiles land in the shared cache")
    func downloadWritesToCache() async {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("offcache-\(UUID().uuidString)")
        let cache = OfflineTileCache(root: root)
        let manager = DiskOfflineTileManager(cacheDirectory: root, fetch: { _ in Data([0xFF, 0xD8]) })
        let bounds = GeoBounds(south: 69.50, west: 19.90, north: 69.52, east: 19.92)
        await manager.download(name: "R", base: .norgeskart, bounds: bounds, minZoom: 6, maxZoom: 6)

        // At least one tile for this region exists at the base-keyed cache path.
        let tile = TileMath.tile(lat: 69.51, lng: 19.91, zoom: 6)
        let url = cache.fileURL(base: .norgeskart, z: tile.z, x: tile.x, y: tile.y)
        #expect(FileManager.default.fileExists(atPath: url.path))
    }

    @Test("the caching overlay serves a cached tile from disk, else the network")
    func overlayResolution() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("offcache-\(UUID().uuidString)")
        let cache = OfflineTileCache(root: root)
        let overlay = CachingTileOverlay(base: .norgeskart, cache: cache)

        // Missing tile → remote template URL.
        let remote = overlay.url(forTilePath: MKTileOverlayPath(x: 1, y: 2, z: 3, contentScaleFactor: 1))
        #expect(remote.scheme == "https")
        #expect(remote.absoluteString.contains("kartverket"))

        // Write a tile to the cache → served from disk.
        let cached = cache.fileURL(base: .norgeskart, z: 3, x: 1, y: 2)
        try FileManager.default.createDirectory(at: cached.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data([0x1]).write(to: cached)
        let local = overlay.url(forTilePath: MKTileOverlayPath(x: 1, y: 2, z: 3, contentScaleFactor: 1))
        #expect(local.isFileURL)
    }
}
