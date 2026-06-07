import Testing
import CoreModel
@testable import CoreData

@Suite("CoreData repositories")
struct RepositoriesTests {

    @Test("marker repository upserts and deletes")
    func markers() async {
        let repo = InMemoryMarkerRepository(seed: [])
        await repo.upsert(Marker(id: "a", name: "A", kind: .cabin, position: LatLng(lat: 1, lng: 2)))
        var all = await repo.current()
        #expect(all.count == 1)
        // upsert with same id replaces
        await repo.upsert(Marker(id: "a", name: "A2", kind: .cabin, position: LatLng(lat: 1, lng: 2)))
        all = await repo.current()
        #expect(all.count == 1)
        #expect(all[0].name == "A2")
        await repo.delete(id: "a")
        all = await repo.current()
        #expect(all.isEmpty)
    }

    @Test("settings repository applies partial updates")
    func settings() async {
        let repo = InMemorySettingsRepository()
        #expect(await repo.current().metricUnits == true)
        await repo.update { $0.metricUnits = false; $0.themeMode = .dark }
        let s = await repo.current()
        #expect(s.metricUnits == false)
        #expect(s.themeMode == .dark)
    }

    @Test("search filters the corpus, empty query yields nothing")
    func search() async {
        let repo = InMemorySearchRepository()
        #expect(await repo.search("").isEmpty)
        let hits = await repo.search("storv")
        #expect(!hits.isEmpty)
        #expect(hits.allSatisfy { $0.name.lowercased().contains("storv") || $0.description.lowercased().contains("storv") })
    }

    @Test("recents are de-duplicated and most-recent-first")
    func recents() async {
        let repo = InMemorySearchRepository(recents: [])
        await repo.addRecent(RecentSearch(name: "A", sub: "x", lat: 1, lng: 1))
        await repo.addRecent(RecentSearch(name: "B", sub: "x", lat: 2, lng: 2))
        await repo.addRecent(RecentSearch(name: "A", sub: "x", lat: 1, lng: 1))
        var iterator = await repo.recents().makeAsyncIterator()
        let list = await iterator.next() ?? []
        #expect(list.count == 2)
        #expect(list.first?.name == "A")
    }

    @Test("path + collection repositories seed sample data")
    func seeds() async {
        #expect(await InMemoryPathRepository().current().count == 5)
        #expect(await InMemoryCollectionRepository().current().count == 3)
    }
}
