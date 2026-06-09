import XCTest

/// End-to-end tests of Turbo's **core user flows**. Each test is named for a goal
/// a hiker actually has, and asserts only on the *outcome* of that goal — not on
/// widget mechanics. The app is launched with `-uitest`, which runs it on
/// deterministic, seeded in-memory backends (no live network or OAuth), so the
/// outcomes are repeatable.
@MainActor
final class TurboUserFlowsUITests: XCTestCase {

    override func setUp() { continueAfterFailure = false }

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-uitest"]
        app.launch()
        return app
    }

    // MARK: Goal — open the app and see the map

    func test_hiker_opens_the_app_and_lands_on_the_map() {
        let app = launch()
        // The map home is the home base: the search and add-a-marker affordances are there.
        XCTAssertTrue(app.buttons["map.search"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["map.fab"].exists)
    }

    // MARK: Goal — find a place and choose it as a destination

    func test_hiker_can_find_a_place_by_searching_and_choose_it() {
        let app = launch()
        app.buttons["map.search"].tap()

        let field = app.searchFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText("storv")

        let result = app.staticTexts["Storvikelva"]
        XCTAssertTrue(result.waitForExistence(timeout: 5), "search did not surface a matching place")
        result.tap()

        // Choosing the place returns the hiker to the map (destination selected).
        XCTAssertTrue(app.buttons["map.search"].waitForExistence(timeout: 5))
    }

    // MARK: Goal — save a spot on the map so it can be found later

    func test_hiker_can_save_a_spot_and_it_appears_on_the_map() {
        let app = launch()
        app.buttons["map.fab"].tap()

        XCTAssertTrue(app.navigationBars["New Marker"].waitForExistence(timeout: 5))
        let name = app.textFields["editor.name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        name.typeText("Blåfjell")
        app.buttons["editor.save"].tap()

        // The saved spot is now a marker the hiker can see on the map.
        XCTAssertTrue(app.descendants(matching: .any)["Blåfjell"].waitForExistence(timeout: 5),
                      "the saved marker did not appear on the map")
    }

    // MARK: Goal — review my recorded hikes

    func test_hiker_can_review_their_recorded_hikes() {
        let app = launch()
        openMenu(app)
        app.buttons["menu.paths"].tap()

        XCTAssertTrue(app.navigationBars["Paths"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["Storheia Loop"].waitForExistence(timeout: 5),
                      "the hiker's recorded tracks are not listed")
    }

    // MARK: Goal — export a track to use it elsewhere (Garmin/Strava/…)

    func test_hiker_can_export_a_recorded_track() {
        let app = launch()
        openMenu(app)
        app.buttons["menu.paths"].tap()

        let track = app.staticTexts["Storheia Loop"]
        XCTAssertTrue(track.waitForExistence(timeout: 5))
        track.press(forDuration: 1.0)   // context menu

        // The hiker can get their track out in a standard format.
        let exportGPX = app.buttons["Export GPX"]
        XCTAssertTrue(exportGPX.waitForExistence(timeout: 5), "no export option offered for the track")
        exportGPX.tap()

        // The system share sheet appears, ready to send the file.
        let share = app.otherElements["ActivityListView"]
        XCTAssertTrue(share.waitForExistence(timeout: 5) || app.buttons["Copy"].waitForExistence(timeout: 5),
                      "the share sheet did not open for the exported track")
    }

    // MARK: Goal — choose the map best suited to the terrain (and have it stick)

    func test_hiker_can_switch_the_base_map_and_the_choice_sticks() {
        let app = launch()
        app.buttons["map.layers"].tap()

        let satellite = app.buttons["Satellite"]
        XCTAssertTrue(satellite.waitForExistence(timeout: 5))
        satellite.tap()
        app.buttons["Done"].tap()

        // Reopening the sheet shows the hiker's choice was remembered.
        app.buttons["map.layers"].tap()
        XCTAssertTrue(app.buttons["Satellite"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["Satellite"].isSelected, "the base-map choice was not remembered")
    }

    // MARK: Goal — download a region so the map works out of signal

    func test_hiker_can_download_a_region_for_offline_use() {
        let app = launch()
        openMenu(app)
        app.buttons["menu.offline"].tap()

        XCTAssertTrue(app.navigationBars["Offline Maps"].waitForExistence(timeout: 5))
        app.buttons["Download This Area"].tap()

        // The area the hiker was viewing is now in their offline maps (named by
        // its coordinates when no place name resolves).
        let region = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "Area ")).firstMatch
        XCTAssertTrue(region.waitForExistence(timeout: 10),
                      "the viewed area was not added to offline maps")
    }

    // MARK: Goal — set a preference and have the app remember it

    func test_hiker_preferences_are_remembered() {
        let app = launch()
        openMenu(app)
        app.buttons["menu.settings"].tap()

        let metric = app.switches["Metric Units"]
        XCTAssertTrue(metric.waitForExistence(timeout: 5))
        let original = (metric.value as? String) ?? "1"
        // Toggle the preference (tap the switch knob, not the row label).
        metric.coordinate(withNormalizedOffset: CGVector(dx: 0.92, dy: 0.5)).tap()
        let changed = XCTNSPredicateExpectation(predicate: NSPredicate(format: "value != %@", original), object: metric)
        XCTAssertEqual(XCTWaiter().wait(for: [changed], timeout: 5), .completed)
        let newValue = (metric.value as? String) ?? ""

        // Leave settings and come back — the preference the hiker set is still set.
        app.navigationBars["Settings"].buttons.firstMatch.tap()   // back to the map
        openMenu(app)
        app.buttons["menu.settings"].tap()
        let reopened = app.switches["Metric Units"]
        XCTAssertTrue(reopened.waitForExistence(timeout: 5))
        XCTAssertEqual(reopened.value as? String, newValue, "the preference was not remembered")
    }

    // MARK: Goal — sign in to my account

    func test_hiker_can_sign_in_to_their_account() {
        let app = launch()
        openMenu(app)
        // Signed out to start.
        let account = app.buttons["menu.account"]
        XCTAssertTrue(account.waitForExistence(timeout: 5))
        XCTAssertTrue(account.label.contains("Not signed in"))
        account.tap()   // open the account screen via the header

        let google = app.buttons["auth.google"]
        XCTAssertTrue(google.waitForExistence(timeout: 5))
        google.tap()

        // Back on the map, signed in — the account now shows the hiker's identity.
        openMenu(app)
        XCTAssertTrue(app.staticTexts["sigmund@granaas.no"].waitForExistence(timeout: 5),
                      "the hiker was not signed in")
    }

    // MARK: Goal — searching for a place takes me there on the map

    func test_searching_for_a_place_recenters_the_map_on_it() {
        let app = launch()
        // The live camera center is published by an invisible probe element.
        let center = app.staticTexts["map.center"]
        XCTAssertTrue(center.waitForExistence(timeout: 10))
        let before = center.label

        search(app, for: "storv", pick: "Storvikelva")

        // The app names the place it took me to…
        XCTAssertTrue(app.staticTexts["Storvikelva"].waitForExistence(timeout: 5),
                      "the chosen place was not shown on the map")
        // …and the map actually recentered there (camera moved).
        let moved = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label != %@", before), object: center)
        XCTAssertEqual(XCTWaiter().wait(for: [moved], timeout: 5), .completed, "the map did not recenter on the place")
    }

    // MARK: Goal — start recording from the map and keep using the map

    func test_hiker_can_start_recording_from_the_map_and_minimize_it() {
        let app = launch()

        // Start a recording straight from the map's control rail.
        XCTAssertTrue(app.buttons["map.record"].waitForExistence(timeout: 10))
        app.buttons["map.record"].tap()
        XCTAssertTrue(app.buttons["recording.stop"].waitForExistence(timeout: 5))

        // Minimize — the session keeps running and the map shows an ambient pill.
        app.buttons["recording.minimize"].tap()
        XCTAssertTrue(app.buttons["map.recording"].waitForExistence(timeout: 5),
                      "the recording pill should appear on the map after minimizing")

        // The pill reopens the same session; stop and discard to finish.
        app.buttons["map.recording"].tap()
        XCTAssertTrue(app.buttons["recording.stop"].waitForExistence(timeout: 5))
        app.buttons["recording.stop"].tap()

        let alert = app.alerts.firstMatch
        XCTAssertTrue(alert.waitForExistence(timeout: 5))
        alert.buttons["Discard"].tap()

        // Session ended — the pill is gone and the start control is back.
        XCTAssertTrue(app.buttons["map.record"].waitForExistence(timeout: 5),
                      "the record control should return once the session ends")
    }

    // MARK: Goal — record a hike and save it

    func test_hiker_can_record_a_track_and_save_it() {
        let app = launch()
        openMenu(app)
        app.buttons["menu.paths"].tap()
        XCTAssertTrue(app.navigationBars["Paths"].waitForExistence(timeout: 5))

        app.buttons["paths.record"].tap()
        XCTAssertTrue(app.buttons["recording.stop"].waitForExistence(timeout: 5))

        // Wait until the track has actually moved (distance > 0).
        let distance = app.staticTexts["recording.distance"]
        let moved = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label != %@", "0.00"), object: distance)
        XCTAssertEqual(XCTWaiter().wait(for: [moved], timeout: 5), .completed, "recording captured no movement")

        app.buttons["recording.stop"].tap()

        let alert = app.alerts.firstMatch
        XCTAssertTrue(alert.waitForExistence(timeout: 5))
        let field = alert.textFields.firstMatch
        field.tap()
        field.typeText("Morning hike")
        alert.buttons["Save"].tap()

        // Back in Paths, the recorded hike is saved and listed.
        XCTAssertTrue(app.staticTexts["Morning hike"].waitForExistence(timeout: 5),
                      "the recorded track was not saved to Paths")
    }

    // MARK: Goal — see the details of a recorded hike

    func test_hiker_can_open_a_hike_detail() {
        let app = launch()
        openMenu(app)
        app.buttons["menu.paths"].tap()
        let row = app.cells.containing(.staticText, identifier: "Storheia Loop").firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 5))
        row.tap()
        XCTAssertTrue(app.navigationBars["Storheia Loop"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["Distance"].waitForExistence(timeout: 5), "hike stats not shown")
    }

    // MARK: Goal — see the details of a saved marker

    func test_hiker_can_open_a_marker_detail() {
        let app = launch()
        openMenu(app)
        app.buttons["menu.markers"].tap()
        let row = app.cells.containing(.staticText, identifier: "Heggmotinden").firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 5))
        row.tap()
        XCTAssertTrue(app.navigationBars["Heggmotinden"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["Coordinate"].waitForExistence(timeout: 5), "marker detail not shown")
    }

    // MARK: Goal — create a collection to organise my places

    func test_hiker_can_create_a_collection() {
        let app = launch()
        openMenu(app)
        app.buttons["menu.collections"].tap()
        XCTAssertTrue(app.navigationBars["Collections"].waitForExistence(timeout: 5))

        app.buttons["collections.new"].tap()
        let field = app.alerts.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText("Autumn trips")
        app.alerts.buttons["Create"].tap()

        XCTAssertTrue(app.staticTexts["Autumn trips"].waitForExistence(timeout: 5),
                      "the new collection was not created")
    }

    // MARK: Goal — browse the spots I've saved

    func test_hiker_can_browse_their_saved_markers() {
        let app = launch()
        openMenu(app)
        app.buttons["menu.markers"].tap()

        XCTAssertTrue(app.navigationBars["My Markers"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["Heggmotinden"].waitForExistence(timeout: 5),
                      "saved markers are not browsable")
    }

    // MARK: Goal — a full trip: find a place, save it, see it listed, export it

    func test_journey_search_then_save_then_browse_then_export() {
        let app = launch()

        // 1. Find a place and go there.
        search(app, for: "Tromsdal", pick: "Tromsdalstinden")
        XCTAssertTrue(app.staticTexts["Tromsdalstinden"].waitForExistence(timeout: 5))

        // 2. Save that place (editor prefilled from the search result).
        app.buttons["focus.save"].tap()
        XCTAssertTrue(app.navigationBars["New Marker"].waitForExistence(timeout: 5))
        XCTAssertEqual(app.textFields["editor.name"].value as? String, "Tromsdalstinden",
                       "the marker editor was not prefilled with the searched place")
        app.buttons["editor.save"].tap()

        // 3. See it listed in My Markers.
        openMenu(app)
        app.buttons["menu.markers"].tap()
        let saved = app.staticTexts["Tromsdalstinden"]
        XCTAssertTrue(saved.waitForExistence(timeout: 5), "the saved place is not in My Markers")

        // 4. Export it.
        saved.press(forDuration: 1.0)
        let exportGPX = app.buttons["Export GPX"]
        XCTAssertTrue(exportGPX.waitForExistence(timeout: 5))
        exportGPX.tap()
        XCTAssertTrue(app.otherElements["ActivityListView"].waitForExistence(timeout: 5)
                      || app.buttons["Copy"].waitForExistence(timeout: 5),
                      "the share sheet did not open")
    }

    // Route building enters via a map long-press, which is too flaky to drive
    // reliably in XCUITest — the route flow (modes, solve, save) is covered by
    // RouteViewModel / RouteSse unit tests instead.

    // MARK: Goal — check the weather and avalanche danger for where I'm headed

    func test_hiker_can_check_weather_and_avalanche_danger() {
        let app = launch()
        app.buttons["map.weather"].tap()
        XCTAssertTrue(app.navigationBars["Weather"].waitForExistence(timeout: 5), "weather forecast not shown")

        let avalanche = app.buttons["weather.avalanche"]
        XCTAssertTrue(avalanche.waitForExistence(timeout: 5))
        avalanche.tap()
        XCTAssertTrue(app.navigationBars["Avalanche Danger"].waitForExistence(timeout: 5), "avalanche detail not shown")
    }

    // MARK: - Helpers

    private func search(_ app: XCUIApplication, for query: String, pick result: String) {
        app.buttons["map.search"].tap()
        let field = app.searchFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText(query)
        let row = app.staticTexts[result]
        XCTAssertTrue(row.waitForExistence(timeout: 5), "search did not surface \(result)")
        row.tap()
    }

    private func openMenu(_ app: XCUIApplication) {
        let avatar = app.buttons["map.avatar"]
        XCTAssertTrue(avatar.waitForExistence(timeout: 10))
        avatar.tap()
    }
}
