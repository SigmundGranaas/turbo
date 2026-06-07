import Foundation
import CoreModel
import CoreCommon

/// User collections (folders of markers/tracks). Mirrors `core.data.CollectionRepository`.
public protocol CollectionRepository: Sendable {
    func current() async -> [MapCollection]
    func stream() async -> AsyncStream<[MapCollection]>
    func upsert(_ collection: MapCollection) async
    func delete(id: String) async
}

public final class InMemoryCollectionRepository: CollectionRepository {
    private let store: ReactiveStore<[MapCollection]>

    public init(seed: [MapCollection] = InMemoryCollectionRepository.sample) {
        store = ReactiveStore(seed)
    }

    public func current() async -> [MapCollection] { await store.current() }
    public func stream() async -> AsyncStream<[MapCollection]> { await store.stream() }

    public func upsert(_ collection: MapCollection) async {
        await store.update { items in
            var next = items.filter { $0.id != collection.id }
            next.append(collection)
            return next
        }
    }

    public func delete(id: String) async {
        await store.update { $0.filter { $0.id != id } }
    }

    public static let sample: [MapCollection] = [
        MapCollection(id: "c1", name: "Summer 2026", icon: "sun.max.fill", itemCount: 14),
        MapCollection(id: "c2", name: "Ski touring", icon: "figure.skiing.downhill", itemCount: 9),
        MapCollection(id: "c3", name: "Fishing spots", icon: "fish.fill", itemCount: 22),
    ]
}
