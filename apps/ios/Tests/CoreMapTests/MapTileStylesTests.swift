import Testing
import CoreModel
@testable import CoreMap

@Suite("MapTileStyles")
struct MapTileStylesTests {

    @Test("tile templates carry x/y/z placeholders with the right axis order")
    func templates() {
        let topo = MapTileStyles.tileURLTemplate(for: .norgeskart)
        #expect(topo.contains("cache.kartverket.no"))
        #expect(topo.hasSuffix("/{z}/{y}/{x}.png"))   // Kartverket: z/y/x

        let osm = MapTileStyles.tileURLTemplate(for: .osm)
        #expect(osm.hasSuffix("/{z}/{x}/{y}.png"))    // OSM: z/x/y

        let sat = MapTileStyles.tileURLTemplate(for: .satellite)
        #expect(sat.contains("arcgisonline"))
        #expect(sat.hasSuffix("/{z}/{y}/{x}"))        // Esri: z/y/x
    }

    @Test("every base layer has attribution")
    func attribution() {
        for layer in BaseLayer.allCases {
            #expect(!MapTileStyles.attribution(for: layer).isEmpty)
        }
    }

    @Test("renderable overlays have tile templates; others don't")
    func overlayTemplates() {
        #expect(MapTileStyles.overlayTemplate(for: .trails)?.contains("waymarkedtrails") == true)
        #expect(MapTileStyles.overlayTemplate(for: .avalanche)?.contains("nve.no") == true)
        // Wave/wind need keyed sources we don't ship — no dead toggles.
        #expect(MapTileStyles.overlayTemplate(for: .waves) == nil)
        #expect(MapTileStyles.overlayTemplate(for: .wind) == nil)
        #expect(MapTileStyles.renderableOverlays == [.trails, .avalanche])
    }
}
