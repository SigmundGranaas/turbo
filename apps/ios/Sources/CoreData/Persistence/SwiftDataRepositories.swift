import Foundation
import SwiftData
import CoreModel

// SwiftData-backed repositories. Each is an `actor` owning its own `ModelContext`
// (never escapes the actor, so it stays concurrency-safe), mapping entities to the
// public `CoreModel` structs. The reactive `stream()` re-emits the full list after
// every mutation — the actor analogue of Room's `Flow<List<…>>`.

public actor SwiftDataMarkerRepository: MarkerRepository {
    private let context: ModelContext
    private var continuations: [UUID: AsyncStream<[Marker]>.Continuation] = [:]

    public init(container: ModelContainer) { context = ModelContext(container) }

    public func current() -> [Marker] {
        ((try? context.fetch(FetchDescriptor<MarkerEntity>())) ?? [])
            .map(\.domain).sorted { $0.name < $1.name }
    }

    public func stream() -> AsyncStream<[Marker]> {
        AsyncStream { continuation in
            let key = UUID()
            continuations[key] = continuation
            continuation.yield(current())
            continuation.onTermination = { [weak self] _ in Task { await self?.drop(key) } }
        }
    }

    public func upsert(_ marker: Marker) {
        let id = marker.id
        if let existing = try? context.fetch(FetchDescriptor<MarkerEntity>(predicate: #Predicate { $0.id == id })).first {
            existing.apply(marker)
        } else {
            context.insert(MarkerEntity(marker))
        }
        try? context.save()
        emit()
    }

    public func delete(id: String) {
        if let existing = try? context.fetch(FetchDescriptor<MarkerEntity>(predicate: #Predicate { $0.id == id })).first {
            context.delete(existing)
            try? context.save()
            emit()
        }
    }

    private func emit() { for c in continuations.values { c.yield(current()) } }
    private func drop(_ key: UUID) { continuations[key] = nil }
}

public actor SwiftDataPathRepository: PathRepository {
    private let context: ModelContext
    private var continuations: [UUID: AsyncStream<[SavedPath]>.Continuation] = [:]

    public init(container: ModelContainer) { context = ModelContext(container) }

    public func current() -> [SavedPath] {
        ((try? context.fetch(FetchDescriptor<PathEntity>())) ?? []).map(\.domain)
    }

    public func stream() -> AsyncStream<[SavedPath]> {
        AsyncStream { continuation in
            let key = UUID()
            continuations[key] = continuation
            continuation.yield(current())
            continuation.onTermination = { [weak self] _ in Task { await self?.drop(key) } }
        }
    }

    public func upsert(_ path: SavedPath) {
        let id = path.id
        if let existing = try? context.fetch(FetchDescriptor<PathEntity>(predicate: #Predicate { $0.id == id })).first {
            existing.apply(path)
        } else {
            context.insert(PathEntity(path))
        }
        try? context.save()
        emit()
    }

    public func delete(id: String) {
        if let existing = try? context.fetch(FetchDescriptor<PathEntity>(predicate: #Predicate { $0.id == id })).first {
            context.delete(existing)
            try? context.save()
            emit()
        }
    }

    private func emit() { for c in continuations.values { c.yield(current()) } }
    private func drop(_ key: UUID) { continuations[key] = nil }
}

public actor SwiftDataCollectionRepository: CollectionRepository {
    private let context: ModelContext
    private var continuations: [UUID: AsyncStream<[MapCollection]>.Continuation] = [:]

    public init(container: ModelContainer) { context = ModelContext(container) }

    public func current() -> [MapCollection] {
        ((try? context.fetch(FetchDescriptor<CollectionEntity>())) ?? []).map(\.domain).sorted { $0.name < $1.name }
    }

    public func stream() -> AsyncStream<[MapCollection]> {
        AsyncStream { continuation in
            let key = UUID()
            continuations[key] = continuation
            continuation.yield(current())
            continuation.onTermination = { [weak self] _ in Task { await self?.drop(key) } }
        }
    }

    public func upsert(_ collection: MapCollection) {
        let id = collection.id
        if let existing = try? context.fetch(FetchDescriptor<CollectionEntity>(predicate: #Predicate { $0.id == id })).first {
            existing.apply(collection)
        } else {
            context.insert(CollectionEntity(collection))
        }
        try? context.save()
        emit()
    }

    public func delete(id: String) {
        if let existing = try? context.fetch(FetchDescriptor<CollectionEntity>(predicate: #Predicate { $0.id == id })).first {
            context.delete(existing)
            try? context.save()
            emit()
        }
    }

    private func emit() { for c in continuations.values { c.yield(current()) } }
    private func drop(_ key: UUID) { continuations[key] = nil }
}
