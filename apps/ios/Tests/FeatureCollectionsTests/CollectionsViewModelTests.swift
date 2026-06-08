import Testing
import CoreModel
import CoreData
@testable import FeatureCollections

@Suite("CollectionsViewModel")
@MainActor
struct CollectionsViewModelTests {

    @Test("create adds a new collection; blank names are ignored")
    func create() async {
        let repo = InMemoryCollectionRepository(seed: [])
        let vm = CollectionsViewModel(repository: repo)
        vm.start()
        vm.create(name: "Summer trips")
        vm.create(name: "   ")
        try? await Task.sleep(for: .milliseconds(150))
        let all = await repo.current()
        #expect(all.count == 1)
        #expect(all.first?.name == "Summer trips")
    }

    @Test("delete removes a collection")
    func delete() async {
        let repo = InMemoryCollectionRepository(seed: [MapCollection(id: "c1", name: "X", itemCount: 0)])
        let vm = CollectionsViewModel(repository: repo)
        vm.start()
        vm.delete(id: "c1")
        try? await Task.sleep(for: .milliseconds(150))
        #expect(await repo.current().isEmpty)
    }
}
