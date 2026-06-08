import Testing
import Foundation
import CoreModel
@testable import CoreData

@Suite("RouteSse")
struct RouteSseTests {

    @Test("request body uses GeoJSON [lon,lat] order + preset key")
    func encode() throws {
        let body = RouteSse.encodeRequest(
            points: [LatLng(lat: 69.6, lng: 20.0), LatLng(lat: 69.7, lng: 20.1)],
            preset: .avoidRoads, profile: "foot"
        )
        let obj = try JSONSerialization.jsonObject(with: body) as! [String: Any]
        let points = obj["points"] as! [[Double]]
        #expect(points[0] == [20.0, 69.6])      // [lon, lat]
        #expect(obj["preset"] as? String == "avoid_roads")
        #expect(obj["profile"] as? String == "foot")
    }

    @Test("progress frame → Progress with lat/lng coordinates")
    func progress() {
        let event = RouteSse.parse(event: "progress",
                                   data: #"{"coordinates":[[20.0,69.6],[20.1,69.7]]}"#)
        guard case let .progress(coords) = event else { Issue.record("not progress"); return }
        #expect(coords.count == 2)
        #expect(coords[0].lat == 69.6)
        #expect(coords[0].lng == 20.0)
    }

    @Test("result frame → Result with plan stats + geometry")
    func result() {
        let data = #"""
        {"distance_m":1234.5,"duration_s":900,"ascent_m":120,"on_trail_pct":0.8,
         "surfaces":{"trail":0.8,"road":0.2},"geometry":{"coordinates":[[20.0,69.6],[20.1,69.7]]}}
        """#
        guard case let .result(plan) = RouteSse.parse(event: "result", data: data) else {
            Issue.record("not result"); return
        }
        #expect(plan.distanceM == 1234.5)
        #expect(plan.onTrailPct == 0.8)
        #expect(plan.geometry.count == 2)
        #expect(plan.geometry[1].lat == 69.7)
    }

    @Test("error frame → Failure; unknown events → nil")
    func errorAndUnknown() {
        guard case let .failure(msg) = RouteSse.parse(event: "error", data: #"{"error":"no route"}"#) else {
            Issue.record("not failure"); return
        }
        #expect(msg == "no route")
        #expect(RouteSse.parse(event: "keepalive", data: "{}") == nil)
        #expect(RouteSse.parse(event: nil, data: "{}") == nil)
    }
}
