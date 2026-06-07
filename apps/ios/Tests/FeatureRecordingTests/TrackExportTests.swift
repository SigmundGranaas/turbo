import Testing
import Foundation
import CoreModel
@testable import FeatureRecording

@Suite("TrackExport")
struct TrackExportTests {

    private func sample(name: String = "Storheia Loop") -> SavedPath {
        SavedPath(
            id: "p1",
            name: name,
            path: GeoPath(
                points: [
                    LatLng(lat: 69.6, lng: 20.0),
                    LatLng(lat: 69.61, lng: 20.02),
                    LatLng(lat: 69.62, lng: 20.05),
                ],
                source: .recording,
                elevations: [10, 24, 18]
            ),
            activityKind: .hiking
        )
    }

    @Test("GPX is valid 1.1 with a trkpt + ele per point")
    func gpx() {
        let xml = TrackExport.serialize(sample(), as: .gpx)
        #expect(xml.contains(#"<?xml version="1.0" encoding="UTF-8"?>"#))
        #expect(xml.contains(#"<gpx version="1.1""#))
        #expect(xml.contains("<name>Storheia Loop</name>"))
        #expect(xml.contains(#"<trkpt lat="69.6" lon="20.0">"#))
        #expect(xml.contains("<ele>24.0</ele>"))
        // three points → three trkpt elements
        #expect(xml.components(separatedBy: "<trkpt").count - 1 == 3)
    }

    @Test("GPX omits <ele> when the track has no elevations")
    func gpxNoElevation() {
        let path = SavedPath(
            id: "p2", name: "Flat",
            path: GeoPath(points: [LatLng(lat: 1, lng: 2)], source: .recording, elevations: nil)
        )
        let xml = TrackExport.serialize(path, as: .gpx)
        #expect(!xml.contains("<ele>"))
    }

    @Test("GPX escapes XML special characters in the name")
    func gpxEscaping() {
        let xml = TrackExport.serialize(sample(name: "Tom & <Jerry>"), as: .gpx)
        #expect(xml.contains("<name>Tom &amp; &lt;Jerry&gt;</name>"))
        #expect(!xml.contains("Tom & <Jerry>"))
    }

    @Test("GeoJSON is a LineString with [lng, lat, ele] coordinates")
    func geojson() {
        let json = TrackExport.serialize(sample(), as: .geojson)
        #expect(json.contains(#""type":"LineString""#))
        #expect(json.contains("[20.0,69.6,10.0]"))
        #expect(json.contains(#""name":"Storheia Loop""#))
    }

    @Test("KML is a Placemark LineString with lng,lat,ele tuples")
    func kml() {
        let kml = TrackExport.serialize(sample(), as: .kml)
        #expect(kml.contains("<kml"))
        #expect(kml.contains("<LineString>"))
        #expect(kml.contains("20.0,69.6,10.0"))
    }

    @Test("file names are sanitised and carry the format extension")
    func fileNames() {
        #expect(TrackExport.fileName(for: "Storheia Loop", format: .gpx) == "Storheia_Loop.gpx")
        #expect(TrackExport.fileName(for: "  ", format: .geojson) == "track.geojson")
        #expect(TrackExport.fileName(for: "Tom & Jerry!", format: .kml) == "Tom_Jerry_.kml")
    }

    @Test("export formats expose label/extension/mime")
    func formats() {
        #expect(ExportFormat.gpx.fileExtension == "gpx")
        #expect(ExportFormat.geojson.mimeType == "application/geo+json")
        #expect(ExportFormat.allCases.count == 3)
    }

    @Test("writeTemporaryFile writes the serialized track to a named file")
    func tempFile() throws {
        let url = try TrackExport.writeTemporaryFile(sample(), as: .gpx)
        defer { try? FileManager.default.removeItem(at: url) }
        #expect(url.pathExtension == "gpx")
        #expect(url.lastPathComponent == "Storheia_Loop.gpx")
        let contents = try String(contentsOf: url, encoding: .utf8)
        #expect(contents == TrackExport.serialize(sample(), as: .gpx))
    }
}
