import Testing
import CoreModel
import CoreMap
@testable import FeatureOffline

@Suite("OfflineViewModel")
@MainActor
struct OfflineViewModelTests {

    @Test("start() mirrors the manager's region list")
    func startObservesRegions() async {
        let vm = OfflineViewModel(manager: InMemoryOfflineTileManager())
        vm.start()
        try? await Task.sleep(for: .milliseconds(200))
        #expect(vm.regions.count == 2)
        vm.stop()
    }

    @Test("download adds a new in-progress region")
    func download() async {
        let vm = OfflineViewModel(manager: InMemoryOfflineTileManager(seed: []))
        vm.start()
        let bounds = GeoBounds(south: 69.45, west: 19.8, north: 69.75, east: 20.4)
        vm.download(name: "Lyngen Alps", base: .norgeskart, bounds: bounds, fromZoom: 11)
        try? await Task.sleep(for: .milliseconds(200))
        #expect(vm.regions.contains { $0.name == "Lyngen Alps" })
        vm.stop()
    }

    @Test("delete removes a region")
    func delete() async {
        let vm = OfflineViewModel(manager: InMemoryOfflineTileManager())
        vm.start()
        try? await Task.sleep(for: .milliseconds(200))
        let id = vm.regions[0].id
        vm.delete(id: id)
        try? await Task.sleep(for: .milliseconds(200))
        #expect(vm.regions.contains { $0.id == id } == false)
        vm.stop()
    }
}
