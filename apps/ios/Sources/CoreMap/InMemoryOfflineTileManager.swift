import Foundation
import CoreModel

/// An in-memory ``OfflineTileManager`` that simulates downloads progressing over
/// time. It lets the offline feature run end-to-end (list, download with a live
/// progress bar, cancel, delete) before the real MapKit/MapLibre tile backend is
/// wired in behind the same seam.
///
/// Implemented as an `actor` for safe concurrent access; updates fan out to all
/// ``regionsStream()`` subscribers — the actor analogue of a `MutableStateFlow`.
public actor InMemoryOfflineTileManager: OfflineTileManager {

    private var regions: [OfflineRegionInfo]
    private var nextId: Int64
    private var continuations: [UUID: AsyncStream<[OfflineRegionInfo]>.Continuation] = [:]
    private var downloadTasks: [Int64: Task<Void, Never>] = [:]

    /// - Parameter seed: regions to start with (defaults to a couple of completed
    ///   downloads so the screen isn't empty on first run, matching the design).
    public init(seed: [OfflineRegionInfo] = InMemoryOfflineTileManager.defaultSeed) {
        self.regions = seed.sorted { $0.name < $1.name }
        self.nextId = (seed.map(\.id).max() ?? 0) + 1
    }

    public func currentRegions() -> [OfflineRegionInfo] { regions }

    public func regionsStream() -> AsyncStream<[OfflineRegionInfo]> {
        AsyncStream { continuation in
            let key = UUID()
            continuations[key] = continuation
            continuation.yield(regions)
            continuation.onTermination = { [weak self] _ in
                Task { await self?.removeContinuation(key) }
            }
        }
    }

    public func refresh() { emit() }

    public func download(name: String, base: BaseLayer, bounds: GeoBounds, minZoom: Double, maxZoom: Double) {
        let id = nextId
        nextId += 1
        // Rough tile-count → byte estimate so the size grows believably.
        let estimatedBytes = Int64((maxZoom - minZoom + 1) * 120 * 1_000_000)
        regions.append(
            OfflineRegionInfo(
                id: id, name: name, complete: false, progress: 0,
                sizeBytes: 0, layers: [base]
            )
        )
        sortAndEmit()
        downloadTasks[id] = Task { [weak self] in
            await self?.simulateDownload(id: id, targetBytes: estimatedBytes)
        }
    }

    public func delete(id: Int64) {
        downloadTasks[id]?.cancel()
        downloadTasks[id] = nil
        regions.removeAll { $0.id == id }
        emit()
    }

    // MARK: - Simulation

    private func simulateDownload(id: Int64, targetBytes: Int64) async {
        let steps = 20
        for step in 1...steps {
            try? await Task.sleep(for: .milliseconds(180))
            if Task.isCancelled { return }
            let progress = Double(step) / Double(steps)
            update(id: id) { region in
                OfflineRegionInfo(
                    id: region.id, name: region.name,
                    complete: progress >= 1,
                    progress: progress,
                    sizeBytes: Int64(Double(targetBytes) * progress),
                    layers: region.layers
                )
            }
        }
        downloadTasks[id] = nil
    }

    private func update(id: Int64, _ transform: (OfflineRegionInfo) -> OfflineRegionInfo) {
        guard let index = regions.firstIndex(where: { $0.id == id }) else { return }
        regions[index] = transform(regions[index])
        emit()
    }

    private func sortAndEmit() {
        regions.sort { $0.name < $1.name }
        emit()
    }

    private func emit() {
        for continuation in continuations.values {
            continuation.yield(regions)
        }
    }

    private func removeContinuation(_ key: UUID) {
        continuations[key] = nil
    }

    public static let defaultSeed: [OfflineRegionInfo] = [
        OfflineRegionInfo(id: 1, name: "Tromsø & Kvaløya", complete: true, progress: 1,
                          sizeBytes: 640 * 1_000_000, layers: [.norgeskart, .satellite]),
        OfflineRegionInfo(id: 2, name: "Senja", complete: true, progress: 1,
                          sizeBytes: 380 * 1_000_000, layers: [.norgeskart]),
    ]
}
