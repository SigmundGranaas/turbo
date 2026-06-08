import Foundation
import Observation
import CoreModel
import CoreData

/// Loads, adds and removes the photos attached to a marker. Mirrors the photo
/// side of Android's `feature.photos`.
@MainActor
@Observable
public final class MarkerPhotosViewModel {
    public private(set) var photos: [Photo] = []

    private let repository: PhotoRepository
    private let marker: Marker

    public init(repository: PhotoRepository, marker: Marker) {
        self.repository = repository
        self.marker = marker
    }

    public func load() async {
        photos = await repository.photos(forMarker: marker.id)
    }

    public func add(imageData: Data) async {
        _ = await repository.add(imageData: imageData, markerId: marker.id, position: marker.position)
        await load()
    }

    public func delete(id: String) async {
        await repository.delete(id: id)
        await load()
    }
}
