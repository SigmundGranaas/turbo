import Foundation
import CoreModel

/// Local store for geotagged photos, optionally attached to a marker. Mirrors
/// `core.data.PhotoRepository`.
public protocol PhotoRepository: Sendable {
    func allPhotos() async -> [Photo]
    func photos(forMarker markerId: String) async -> [Photo]
    /// Persist `imageData` (writes the file) and index it; returns the new photo.
    func add(imageData: Data, markerId: String?, position: LatLng) async -> Photo?
    func delete(id: String) async
}

/// File-backed implementation: images live as files in `directory`, with a JSON
/// index of metadata alongside. The real store and a fully-testable one — point
/// `directory` at a temp dir in tests.
public actor FilePhotoRepository: PhotoRepository {
    private let directory: URL
    private let fileManager = FileManager.default
    private let now: @Sendable () -> Int64
    private var index: [Photo]
    private var loaded = false

    public init(directory: URL? = nil, now: @escaping @Sendable () -> Int64 = { Int64(Date().timeIntervalSince1970 * 1000) }) {
        self.directory = directory
            ?? FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
                .appendingPathComponent("turbo-photos", isDirectory: true)
        self.now = now
        self.index = []
    }

    public func allPhotos() -> [Photo] { ensureLoaded(); return index }

    public func photos(forMarker markerId: String) -> [Photo] {
        ensureLoaded()
        return index.filter { $0.markerId == markerId }.sorted { $0.capturedAtEpochMs > $1.capturedAtEpochMs }
    }

    public func add(imageData: Data, markerId: String?, position: LatLng) -> Photo? {
        ensureLoaded()
        let id = UUID().uuidString
        let fileURL = directory.appendingPathComponent("\(id).jpg")
        guard (try? imageData.write(to: fileURL)) != nil else { return nil }
        let photo = Photo(id: id, markerId: markerId, lat: position.lat, lng: position.lng,
                          uri: fileURL.absoluteString, capturedAtEpochMs: now())
        index.append(photo)
        persist()
        return photo
    }

    public func delete(id: String) {
        ensureLoaded()
        if let photo = index.first(where: { $0.id == id }), let url = URL(string: photo.uri) {
            try? fileManager.removeItem(at: url)
        }
        index.removeAll { $0.id == id }
        persist()
    }

    // MARK: - Storage

    private var indexURL: URL { directory.appendingPathComponent("index.json") }

    private func ensureLoaded() {
        guard !loaded else { return }
        try? fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
        if let data = try? Data(contentsOf: indexURL),
           let stored = try? JSONDecoder().decode([Photo].self, from: data) {
            index = stored
        }
        loaded = true
    }

    private func persist() {
        if let data = try? JSONEncoder().encode(index) { try? data.write(to: indexURL) }
    }
}
