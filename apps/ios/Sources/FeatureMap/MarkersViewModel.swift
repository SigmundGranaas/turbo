import Foundation
import Observation
import CoreModel
import CoreData

/// Lists the user's saved markers — the browsable home for everything dropped on
/// the map. Mirrors the marker-list role of Android's `feature.markers`.
@MainActor
@Observable
public final class MarkersViewModel {
    public private(set) var markers: [Marker] = []

    private let repository: MarkerRepository
    private var observation: Task<Void, Never>?

    public init(repository: MarkerRepository) {
        self.repository = repository
    }

    public func start() {
        guard observation == nil else { return }
        observation = Task { [weak self, repository] in
            for await list in await repository.stream() {
                self?.markers = list
            }
        }
    }

    public func stop() { observation?.cancel(); observation = nil }

    public func delete(id: String) {
        Task { [repository] in await repository.delete(id: id) }
    }
}
