import Testing
import CoreModel
import CoreData
@testable import FeatureMap

@Suite("MarkersViewModel")
@MainActor
struct MarkersViewModelTests {

    @Test("start() lists the saved markers from the repository")
    func lists() async {
        let vm = MarkersViewModel(repository: InMemoryMarkerRepository())
        vm.start()
        try? await Task.sleep(for: .milliseconds(150))
        #expect(vm.markers.count == 3)
        vm.stop()
    }

    @Test("delete removes a marker")
    func delete() async {
        let repo = InMemoryMarkerRepository()
        let vm = MarkersViewModel(repository: repo)
        vm.start()
        try? await Task.sleep(for: .milliseconds(150))
        let id = vm.markers[0].id
        vm.delete(id: id)
        try? await Task.sleep(for: .milliseconds(150))
        #expect(vm.markers.contains { $0.id == id } == false)
        vm.stop()
    }
}
