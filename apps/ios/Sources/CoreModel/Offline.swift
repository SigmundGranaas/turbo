import Foundation

/// A downloaded (or downloading) offline map region. Mirrors `domain.OfflineRegionInfo`.
public struct OfflineRegionInfo: Identifiable, Hashable, Sendable {
    public let id: Int64
    public let name: String
    public let complete: Bool
    /// Download progress 0...1 (1 when complete).
    public let progress: Double
    public let sizeBytes: Int64
    /// The base map(s) this region covers — used for the "Topo + Satellite" subtitle.
    public let layers: [BaseLayer]

    public init(
        id: Int64,
        name: String,
        complete: Bool,
        progress: Double,
        sizeBytes: Int64,
        layers: [BaseLayer] = [.norgeskart]
    ) {
        self.id = id
        self.name = name
        self.complete = complete
        self.progress = progress
        self.sizeBytes = sizeBytes
        self.layers = layers
    }
}
