import Testing
@testable import CoreCommon

@Suite("LoadState")
struct LoadStateTests {

    @Test("accessors reflect the case")
    func accessors() {
        #expect(LoadState<Int>.loading.isLoading)
        #expect(LoadState<Int>.loaded(7).value == 7)
        #expect(LoadState<Int>.failed("nope").errorMessage == "nope")
        #expect(LoadState<Int>.idle.value == nil)
        #expect(LoadState<Int>.empty.isLoading == false)
    }

    @Test("resolve(optional:) maps nil to failure, value to loaded")
    func resolveOptional() {
        #expect(LoadState.resolve(42, failure: "x").value == 42)
        let failed = LoadState<Int>.resolve(nil, failure: "Couldn't load")
        #expect(failed.errorMessage == "Couldn't load")
    }

    @Test("resolve(collection:) maps empty to .empty, non-empty to .loaded")
    func resolveCollection() {
        let empty = LoadState<[Int]>.resolve([])
        if case .empty = empty {} else { Issue.record("expected .empty") }
        #expect(LoadState<[Int]>.resolve([1, 2]).value == [1, 2])
    }
}
