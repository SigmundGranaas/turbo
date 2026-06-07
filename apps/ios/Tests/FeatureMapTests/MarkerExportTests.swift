import Testing
import Foundation
import CoreModel
@testable import FeatureMap

@Suite("MarkerExport")
struct MarkerExportTests {

    private func marker() -> Marker {
        Marker(id: "m1", name: "Heggmotinden", kind: .mountain,
               position: LatLng(lat: 69.5502, lng: 19.8801), notes: "Summit")
    }

    @Test("a marker exports as a GPX waypoint with name + coordinate")
    func gpx() {
        let xml = MarkerExport.gpx(marker())
        #expect(xml.contains(#"<gpx version="1.1""#))
        #expect(xml.contains(#"<wpt lat="69.5502" lon="19.8801">"#))
        #expect(xml.contains("<name>Heggmotinden</name>"))
    }

    @Test("special characters in the name are XML-escaped")
    func escaping() {
        let m = Marker(id: "m2", name: "Tom & <Jerry>", kind: .cabin, position: LatLng(lat: 1, lng: 2))
        #expect(MarkerExport.gpx(m).contains("<name>Tom &amp; &lt;Jerry&gt;</name>"))
    }

    @Test("writeTemporaryFile writes a named .gpx file")
    func tempFile() throws {
        let url = try MarkerExport.writeTemporaryFile(marker())
        defer { try? FileManager.default.removeItem(at: url) }
        #expect(url.pathExtension == "gpx")
        #expect(url.lastPathComponent == "Heggmotinden.gpx")
        #expect(try String(contentsOf: url, encoding: .utf8) == MarkerExport.gpx(marker()))
    }
}
