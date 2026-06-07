import Testing
import Foundation
import CoreModel
@testable import CoreMap

@Suite("Offline tiles")
struct OfflineDownloadTests {

    @Test("tile math maps a coordinate to the right slippy tile")
    func tileForCoordinate() {
        // (lng 0, lat 0) at z1 → tile (1,1)
        let tile = TileMath.tile(lat: 0, lng: 0, zoom: 1)
        #expect(tile.x == 1)
        #expect(tile.y == 1)
        #expect(tile.z == 1)
    }

    @Test("a bounds yields at least one tile per zoom level")
    func tilesInBounds() {
        let bounds = GeoBounds(south: 69.50, west: 19.90, north: 69.60, east: 20.05)
        let tiles = TileMath.tiles(in: bounds, minZoom: 5, maxZoom: 7)
        #expect(!tiles.isEmpty)
        #expect(Set(tiles.map(\.z)) == [5, 6, 7])
    }

    @Test("download fetches every tile, reports progress to 1, persists size")
    func downloadCompletes() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("offtest-\(UUID().uuidString)")
        let manager = DiskOfflineTileManager(cacheDirectory: dir, fetch: { _ in Data([0xFF, 0xD8, 0xFF]) })
        let bounds = GeoBounds(south: 69.50, west: 19.90, north: 69.55, east: 19.95)
        await manager.download(name: "Test region", base: .norgeskart, bounds: bounds, minZoom: 5, maxZoom: 6)

        let regions = await manager.currentRegions()
        #expect(regions.count == 1)
        #expect(regions[0].complete)
        #expect(regions[0].progress == 1)
        #expect(regions[0].sizeBytes > 0)
    }

    @Test("regions persist across manager instances sharing a cache dir")
    func persistsRegions() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("offtest-\(UUID().uuidString)")
        let manager = DiskOfflineTileManager(cacheDirectory: dir, fetch: { _ in Data([0x1]) })
        let bounds = GeoBounds(south: 69.50, west: 19.90, north: 69.52, east: 19.92)
        await manager.download(name: "Persisted", base: .norgeskart, bounds: bounds, minZoom: 5, maxZoom: 5)

        let reopened = DiskOfflineTileManager(cacheDirectory: dir, fetch: { _ in nil })
        await reopened.refresh()
        let regions = await reopened.currentRegions()
        #expect(regions.contains { $0.name == "Persisted" })

        await reopened.delete(id: regions[0].id)
        #expect(await reopened.currentRegions().isEmpty)
    }
}
