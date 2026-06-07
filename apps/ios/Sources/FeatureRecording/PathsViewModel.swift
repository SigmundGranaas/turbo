import Foundation
import Observation
import CoreModel
import CoreData

/// Lists recorded/saved tracks. Mirrors `feature.recording.PathsViewModel` (Android).
@MainActor
@Observable
public final class PathsViewModel {
    public private(set) var paths: [SavedPath] = []

    private let repository: PathRepository
    private var observation: Task<Void, Never>?

    public init(repository: PathRepository) {
        self.repository = repository
    }

    public func start() {
        guard observation == nil else { return }
        observation = Task { [weak self, repository] in
            for await list in await repository.stream() {
                self?.paths = list
            }
        }
    }

    public func stop() { observation?.cancel(); observation = nil }

    public func delete(id: String) {
        Task { [repository] in await repository.delete(id: id) }
    }
}
