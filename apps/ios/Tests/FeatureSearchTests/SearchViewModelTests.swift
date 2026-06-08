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
        #expect(vm.state.isLoading)          // shows progress immediately
        try? await Task.sleep(for: .milliseconds(450))   // past the 300ms debounce
        #expect(vm.state.value?.isEmpty == false)        // results arrived

        vm.query = ""
        vm.runSearch()
        if case .idle = vm.state {} else { Issue.record("empty query should reset to .idle") }
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
