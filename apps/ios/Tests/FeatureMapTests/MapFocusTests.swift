import Testing
import CoreModel
import CoreData
@testable import FeatureMap

@Suite("Map focus + editor prefill")
@MainActor
struct MapFocusTests {

    @Test("focusing on a search result sets the focused place; clearing resets it")
    func focus() {
        let vm = MapViewModel(markerRepository: InMemoryMarkerRepository(seed: []))
        #expect(vm.focusedPlace == nil)
        vm.focus(on: LatLng(lat: 69.6, lng: 20.1), name: "Storvikelva")
        #expect(vm.focusedPlace?.name == "Storvikelva")
        #expect(vm.focusedPlace?.position.lat == 69.6)
        vm.clearFocus()
        #expect(vm.focusedPlace == nil)
    }

    @Test("an editor for a focused place is prefilled with its name + position")
    func editorPrefill() {
        let vm = MapViewModel(markerRepository: InMemoryMarkerRepository(seed: []))
        let editor = vm.makeEditor(at: LatLng(lat: 1, lng: 2), name: "Storvikelva")
        #expect(editor.name == "Storvikelva")
        #expect(editor.position.lng == 2)
    }
}
