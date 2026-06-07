import Foundation
import CoreModel

/// A real ``OfflineTileManager`` that downloads XYZ raster tiles to a cache
/// directory and tracks region metadata on disk — the iOS-native equivalent of
/// MapLibre's `OfflineManager` (which has no MapKit counterpart, so we roll the
/// tile pyramid ourselves). The `fetch` closure is injectable so downloads can be
/// unit-tested without the network.
///
/// Tiles land at `<cache>/tiles/<baseId>/<z>/<x>/<y>`; region metadata is a JSON
/// sidecar so regions survive relaunch. `TurboMapView` can later read these
/// cached tiles first for true offline rendering.
public actor DiskOfflineTileManager: OfflineTileManager {
    public typealias Fetch = @Sendable (URL) async -> Data?

    private let cacheDirectory: URL
    private let fetch: Fetch
    private let fileManager = FileManager.default
    private var regions: [OfflineRegionInfo] = []
    private var continuations: [UUID: AsyncStream<[OfflineRegionInfo]>.Continuation] = [:]
    private var nextId: Int64 = 1
    private var loaded = false

    public init(cacheDirectory: URL? = nil, fetch: @escaping Fetch = DiskOfflineTileManager.urlSessionFetch) {
        self.cacheDirectory = cacheDirectory
            ?? FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
                .appendingPathComponent("turbo-offline", isDirectory: true)
        self.fetch = fetch
    }

    public func currentRegions() -> [OfflineRegionInfo] {
        ensureLoaded()
        return regions
    }

    public func regionsStream() -> AsyncStream<[OfflineRegionInfo]> {
        ensureLoaded()
        return AsyncStream { continuation in
            let key = UUID()
            continuations[key] = continuation
            continuation.yield(regions)
            continuation.onTermination = { [weak self] _ in Task { await self?.drop(key) } }
        }
    }

    public func refresh() {
        loaded = false
        ensureLoaded()
        emit()
    }

    public func download(name: String, base: BaseLayer, bounds: GeoBounds, minZoom: Double, maxZoom: Double) async {
        ensureLoaded()
        let id = nextId
        nextId += 1
        let tiles = TileMath.tiles(in: bounds, minZoom: Int(minZoom), maxZoom: Int(maxZoom))
        let template = MapTileStyles.tileURLTemplate(for: base)
        let regionDir = tilesRoot.appendingPathComponent("\(id)", isDirectory: true)
        try? fileManager.createDirectory(at: regionDir, withIntermediateDirectories: true)

        upsertRegion(OfflineRegionInfo(id: id, name: name, complete: false, progress: 0, sizeBytes: 0, layers: [base]))

        var downloaded = 0
        var bytes: Int64 = 0
        for (index, tile) in tiles.enumerated() {
            guard let url = TileMath.url(template: template, for: tile), let data = await fetch(url) else { continue }
            let tileURL = regionDir.appendingPathComponent("\(tile.z)_\(tile.x)_\(tile.y)")
            try? data.write(to: tileURL)
            downloaded += 1
            bytes += Int64(data.count)
            // Emit progress every few tiles so the UI bar advances.
            if index % 8 == 0 || index == tiles.count - 1 {
                let progress = Double(index + 1) / Double(max(tiles.count, 1))
                upsertRegion(OfflineRegionInfo(id: id, name: name, complete: false, progress: progress, sizeBytes: bytes, layers: [base]))
            }
        }
        upsertRegion(OfflineRegionInfo(id: id, name: name, complete: true, progress: 1, sizeBytes: bytes, layers: [base]))
        persist()
    }

    public func delete(id: Int64) {
        ensureLoaded()
        regions.removeAll { $0.id == id }
        try? fileManager.removeItem(at: tilesRoot.appendingPathComponent("\(id)", isDirectory: true))
        persist()
        emit()
    }

    // MARK: - Storage

    private var tilesRoot: URL { cacheDirectory.appendingPathComponent("tiles", isDirectory: true) }
    private var metadataURL: URL { cacheDirectory.appendingPathComponent("regions.json") }

    private func ensureLoaded() {
        guard !loaded else { return }
        try? fileManager.createDirectory(at: cacheDirectory, withIntermediateDirectories: true)
        if let data = try? Data(contentsOf: metadataURL),
           let stored = try? JSONDecoder().decode([StoredRegion].self, from: data) {
            regions = stored.map(\.info)
            nextId = (regions.map(\.id).max() ?? 0) + 1
        }
        loaded = true
    }

    private func upsertRegion(_ info: OfflineRegionInfo) {
        regions.removeAll { $0.id == info.id }
        regions.append(info)
        regions.sort { $0.name < $1.name }
        emit()
    }

    private func persist() {
        let stored = regions.map(StoredRegion.init)
        if let data = try? JSONEncoder().encode(stored) { try? data.write(to: metadataURL) }
    }

    private func emit() { for c in continuations.values { c.yield(regions) } }
    private func drop(_ key: UUID) { continuations[key] = nil }

    /// Default network fetcher.
    public static let urlSessionFetch: Fetch = { url in
        try? await URLSession.shared.data(from: url).0
    }

    /// Codable mirror of `OfflineRegionInfo` for the JSON sidecar.
    private struct StoredRegion: Codable {
        let id: Int64
        let name: String
        let complete: Bool
        let progress: Double
        let sizeBytes: Int64
        let layers: [String]

        init(_ i: OfflineRegionInfo) {
            id = i.id; name = i.name; complete = i.complete; progress = i.progress
            sizeBytes = i.sizeBytes; layers = i.layers.map(\.rawValue)
        }
        var info: OfflineRegionInfo {
            OfflineRegionInfo(id: id, name: name, complete: complete, progress: progress,
                              sizeBytes: sizeBytes, layers: layers.compactMap(BaseLayer.init(rawValue:)))
        }
    }
}
