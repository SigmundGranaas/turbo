import Testing
import CoreModel
@testable import CoreMap

@Suite("InMemoryOfflineTileManager")
struct InMemoryOfflineTileManagerTests {

    @Test("seeds with completed regions")
    func seed() async {
        let manager = InMemoryOfflineTileManager()
        let regions = await manager.currentRegions()
        #expect(regions.count == 2)
        let allComplete = regions.allSatisfy { $0.complete }
        #expect(allComplete)
    }

    @Test("download appends a region and drives it to completion")
    func downloadCompletes() async {
        let manager = InMemoryOfflineTileManager(seed: [])
        let bounds = GeoBounds(south: 69.45, west: 19.8, north: 69.75, east: 20.4)
        await manager.download(name: "Lyngen", base: .norgeskart, bounds: bounds, minZoom: 11, maxZoom: 15)

        // Region exists immediately, in-progress.
        var regions = await manager.currentRegions()
        #expect(regions.count == 1)
        #expect(regions[0].complete == false)

        // It completes after the simulated download.
        try? await Task.sleep(for: .seconds(5))
        regions = await manager.currentRegions()
        #expect(regions.count == 1)
        #expect(regions[0].complete)
        #expect(regions[0].progress == 1)
        #expect(regions[0].sizeBytes > 0)
    }

    @Test("delete removes the region")
    func delete() async {
        let manager = InMemoryOfflineTileManager()
        let before = await manager.currentRegions()
        let id = before[0].id
        await manager.delete(id: id)
        let after = await manager.currentRegions()
        #expect(after.contains { $0.id == id } == false)
    }

    @Test("stream emits the latest list to subscribers")
    func streamEmits() async {
        let manager = InMemoryOfflineTileManager(seed: [])
        var iterator = await manager.regionsStream().makeAsyncIterator()
        let first = await iterator.next()
        #expect(first?.isEmpty == true)
    }
}
