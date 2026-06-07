import Foundation
import Observation
import CoreModel
import CoreData

/// Drives the search screen — debounced results from ``SearchRepository`` plus
/// the recent picks shown when the query is empty. Mirrors
/// `feature.search.SearchViewModel` (Android).
@MainActor
@Observable
public final class SearchViewModel {
    public var query: String = ""
    public private(set) var results: [SearchHit] = []
    public private(set) var recents: [RecentSearch] = []

    private let repository: SearchRepository
    private var recentsObservation: Task<Void, Never>?
    private var searchTask: Task<Void, Never>?

    public init(repository: SearchRepository) {
        self.repository = repository
    }

    public func start() {
        guard recentsObservation == nil else { return }
        recentsObservation = Task { [weak self, repository] in
            for await list in await repository.recents() {
                self?.recents = list
            }
        }
    }

    public func stop() {
        recentsObservation?.cancel(); recentsObservation = nil
        searchTask?.cancel(); searchTask = nil
    }

    /// Re-run the search for the current ``query`` (call from `.onChange`).
    public func runSearch() {
        searchTask?.cancel()
        let q = query
        guard !q.trimmingCharacters(in: .whitespaces).isEmpty else {
            results = []
            return
        }
        searchTask = Task { [weak self, repository] in
            let hits = await repository.search(q)
            if !Task.isCancelled { self?.results = hits }
        }
    }

    /// Remember a picked hit so it surfaces in recents next time.
    public func remember(_ hit: SearchHit) {
        Task { [repository] in
            await repository.addRecent(
                RecentSearch(name: hit.name, sub: hit.description, lat: hit.position.lat, lng: hit.position.lng)
            )
        }
    }
}
