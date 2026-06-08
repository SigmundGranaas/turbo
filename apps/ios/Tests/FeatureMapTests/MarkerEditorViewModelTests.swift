import Testing
import CoreModel
import CoreData
@testable import FeatureMap

@Suite("MarkerEditorViewModel")
@MainActor
struct MarkerEditorViewModelTests {

    @Test("a new editor starts empty at the dropped point")
    func newDefaults() {
        let vm = MarkerEditorViewModel(
            repository: InMemoryMarkerRepository(seed: []),
            position: LatLng(lat: 69.6, lng: 20.0)
        )
        #expect(vm.isEditing == false)
        #expect(vm.name.isEmpty)
        #expect(vm.kind == .mountain)
        #expect(vm.position.lat == 69.6)
    }

    @Test("saving a blank name falls back to the kind's label")
    func blankNameFallback() async {
        let repo = InMemoryMarkerRepository(seed: [])
        let vm = MarkerEditorViewModel(repository: repo, position: LatLng(lat: 1, lng: 2))
        vm.kind = .cabin
        vm.name = "   "
        vm.save()
        try? await Task.sleep(for: .milliseconds(120))
        let all = await repo.current()
        #expect(all.count == 1)
        #expect(all[0].name == ActivityKindId.cabin.label)
        #expect(all[0].kind == .cabin)
    }

    @Test("saving keeps a provided name and notes")
    func savesProvided() async {
        let repo = InMemoryMarkerRepository(seed: [])
        let vm = MarkerEditorViewModel(repository: repo, position: LatLng(lat: 1, lng: 2))
        vm.name = "Secret spot"
        vm.kind = .fishing
        vm.notes = "Big trout"
        vm.save()
        try? await Task.sleep(for: .milliseconds(120))
        let marker = await repo.current().first
        #expect(marker?.name == "Secret spot")
        #expect(marker?.kind == .fishing)
        #expect(marker?.notes == "Big trout")
    }

    @Test("a chosen colour is saved as the marker's colorArgb")
    func savesColor() async {
        let repo = InMemoryMarkerRepository(seed: [])
        let vm = MarkerEditorViewModel(repository: repo, position: LatLng(lat: 1, lng: 2))
        vm.name = "Camp"
        vm.colorArgb = 0xFFD32F2F
        vm.save()
        try? await Task.sleep(for: .milliseconds(120))
        #expect(await repo.current().first?.colorArgb == 0xFFD32F2F)
    }

    @Test("editing loads the marker's existing colour")
    func loadsColor() {
        let marker = Marker(id: "m1", name: "X", kind: .cabin, position: LatLng(lat: 1, lng: 1), colorArgb: 0xFF1976D2)
        let vm = MarkerEditorViewModel(repository: InMemoryMarkerRepository(seed: [marker]), marker: marker)
        #expect(vm.colorArgb == 0xFF1976D2)
    }

    @Test("editing an existing marker updates the same row")
    func editExisting() async {
        let existing = Marker(id: "m1", name: "Old", kind: .mountain, position: LatLng(lat: 5, lng: 6), notes: "n")
        let repo = InMemoryMarkerRepository(seed: [existing])
        let vm = MarkerEditorViewModel(repository: repo, marker: existing)
        #expect(vm.isEditing)
        #expect(vm.name == "Old")
        vm.name = "New name"
        vm.save()
        try? await Task.sleep(for: .milliseconds(120))
        let all = await repo.current()
        #expect(all.count == 1)             // replaced, not appended
        #expect(all[0].id == "m1")
        #expect(all[0].name == "New name")
    }

    @Test("delete removes the edited marker")
    func delete() async {
        let existing = Marker(id: "m1", name: "Old", kind: .mountain, position: LatLng(lat: 5, lng: 6))
        let repo = InMemoryMarkerRepository(seed: [existing])
        let vm = MarkerEditorViewModel(repository: repo, marker: existing)
        vm.delete()
        try? await Task.sleep(for: .milliseconds(120))
        #expect(await repo.current().isEmpty)
    }
}
