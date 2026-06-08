import Foundation
import Observation
import CoreModel
import CoreData

/// Lists the user's collections. Mirrors `feature.collections.CollectionsViewModel`.
@MainActor
@Observable
public final class CollectionsViewModel {
    public private(set) var collections: [MapCollection] = []

    private let repository: CollectionRepository
    private var observation: Task<Void, Never>?

    public init(repository: CollectionRepository) {
        self.repository = repository
    }

    public func start() {
        guard observation == nil else { return }
        observation = Task { [weak self, repository] in
            for await list in await repository.stream() {
                self?.collections = list
            }
        }
    }

    public func stop() { observation?.cancel(); observation = nil }

    public func delete(id: String) {
        Task { [repository] in await repository.delete(id: id) }
    }

    /// Create a new (empty) collection. Blank names are ignored.
    public func create(name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let collection = MapCollection(id: "c-\(UUID().uuidString)", name: trimmed, itemCount: 0)
        Task { [repository] in await repository.upsert(collection) }
    }
}
