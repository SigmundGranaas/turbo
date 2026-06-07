import Foundation
import CoreModel
import CoreCommon

/// Place search + recent picks. Mirrors `core.data.SearchRepository` +
/// `RecentSearchRepository`. The real backend queries Kartverket stedsnavn; this
/// in-memory stub filters a fixed corpus so the UI works offline.
public protocol SearchRepository: Sendable {
    func search(_ query: String) async -> [SearchHit]
    func recents() async -> AsyncStream<[RecentSearch]>
    func addRecent(_ recent: RecentSearch) async
}

public final class InMemorySearchRepository: SearchRepository {
    private let recentStore: ReactiveStore<[RecentSearch]>
    private let corpus: [SearchHit]

    public init(
        recents: [RecentSearch] = InMemorySearchRepository.sampleRecents,
        corpus: [SearchHit] = InMemorySearchRepository.sampleCorpus
    ) {
        self.recentStore = ReactiveStore(recents)
        self.corpus = corpus
    }

    public func search(_ query: String) async -> [SearchHit] {
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return [] }
        return corpus.filter { $0.name.lowercased().contains(q) || $0.description.lowercased().contains(q) }
    }

    public func recents() async -> AsyncStream<[RecentSearch]> { await recentStore.stream() }

    public func addRecent(_ recent: RecentSearch) async {
        await recentStore.update { list in
            ([recent] + list.filter { $0.id != recent.id }).prefix(8).map { $0 }
        }
    }

    public static let sampleRecents: [RecentSearch] = [
        RecentSearch(name: "Heggmotinden", sub: "798 m · summit · 4.2 km", lat: 69.55, lng: 19.88),
        RecentSearch(name: "Storvika camp", sub: "Camping · 1.1 km", lat: 69.62, lng: 20.05),
        RecentSearch(name: "Tverrelvvatnet", sub: "Recent search", lat: 69.58, lng: 20.0),
    ]

    public static let sampleCorpus: [SearchHit] = [
        SearchHit(name: "Storelvdalen", description: "Valley · Troms", position: LatLng(lat: 69.6, lng: 19.7), kind: .mountain),
        SearchHit(name: "Storvikelva", description: "Fishing spot · 2.6 km", position: LatLng(lat: 69.60, lng: 20.12), kind: .fishing),
        SearchHit(name: "Heggmotinden", description: "798 m · summit", position: LatLng(lat: 69.55, lng: 19.88), kind: .mountain),
        SearchHit(name: "Tromsdalstinden", description: "1238 m · summit", position: LatLng(lat: 69.61, lng: 19.13), kind: .mountain),
        SearchHit(name: "Storvika camp", description: "Camping · 1.1 km", position: LatLng(lat: 69.62, lng: 20.05), kind: .camping),
    ]
}
