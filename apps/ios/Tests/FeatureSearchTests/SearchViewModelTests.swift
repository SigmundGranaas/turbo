import Testing
import CoreModel
import CoreData
@testable import FeatureSearch

@Suite("SearchViewModel")
@MainActor
struct SearchViewModelTests {

    @Test("runSearch populates results; empty query clears them")
    func search() async {
        let vm = SearchViewModel(repository: InMemorySearchRepository())
        vm.query = "storv"
        vm.runSearch()
        #expect(vm.isSearching)              // shows progress immediately
        try? await Task.sleep(for: .milliseconds(120))
        #expect(!vm.results.isEmpty)
        #expect(!vm.isSearching)             // cleared when results arrive

        vm.query = ""
        vm.runSearch()
        #expect(vm.results.isEmpty)
        #expect(!vm.isSearching)
    }

    @Test("start() surfaces recents")
    func recents() async {
        let vm = SearchViewModel(repository: InMemorySearchRepository())
        vm.start()
        try? await Task.sleep(for: .milliseconds(120))
        #expect(!vm.recents.isEmpty)
        vm.stop()
    }
}
