import Testing
import Foundation
import CoreModel
@testable import CoreData

@Suite("FilePhotoRepository")
struct PhotoRepositoryTests {

    private func tempDir() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent("phototest-\(UUID().uuidString)")
    }

    @Test("adding writes a file and indexes it under its marker")
    func add() async {
        let repo = FilePhotoRepository(directory: tempDir())
        let photo = await repo.add(imageData: Data([0xFF, 0xD8, 0xFF]), markerId: "m1", position: LatLng(lat: 69.6, lng: 20.0))
        #expect(photo != nil)
        // The file exists on disk.
        let url = URL(string: photo!.uri)!
        #expect(FileManager.default.fileExists(atPath: url.path))
        // It's indexed for the marker.
        let forMarker = await repo.photos(forMarker: "m1")
        #expect(forMarker.count == 1)
        #expect(await repo.photos(forMarker: "other").isEmpty)
    }

    @Test("photos persist across repository instances on the same directory")
    func persists() async {
        let dir = tempDir()
        _ = await FilePhotoRepository(directory: dir).add(imageData: Data([0x1]), markerId: "m1", position: LatLng(lat: 1, lng: 2))
        let reopened = FilePhotoRepository(directory: dir)
        #expect(await reopened.photos(forMarker: "m1").count == 1)
    }

    @Test("delete removes the index entry and the file")
    func delete() async {
        let repo = FilePhotoRepository(directory: tempDir())
        let photo = await repo.add(imageData: Data([0x1]), markerId: "m1", position: LatLng(lat: 1, lng: 2))!
        await repo.delete(id: photo.id)
        #expect(await repo.photos(forMarker: "m1").isEmpty)
        #expect(FileManager.default.fileExists(atPath: URL(string: photo.uri)!.path) == false)
    }
}
