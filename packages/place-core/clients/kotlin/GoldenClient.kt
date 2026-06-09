/*
 * Synthetic UniFFI test client (Kotlin / JVM).
 *
 * Drives `place-core` through the generated Kotlin binding to prove the FFI
 * surface lifts/lowers correctly on the JVM before wiring it into the Android
 * app. Uses a representative subset of the golden cases (constructed via the
 * generated data classes, so no JSON dependency) — full fixture parity is
 * proven by the Python client, which replays every case.
 *
 * Run from the crate root (see ../README.md):
 *   kotlinc <binding>.kt GoldenClient.kt -cp jna.jar -include-runtime -d gc.jar
 *   java -cp gc.jar:jna.jar -Djava.library.path=target/debug GoldenClientKt
 */

import uniffi.place_core.Candidate
import uniffi.place_core.EngineException
import uniffi.place_core.Kommune
import uniffi.place_core.PlaceEngine
import uniffi.place_core.PlaceEngineInterface
import uniffi.place_core.ProtectedArea
import uniffi.place_core.Qualifier
import uniffi.place_core.ReverseInput
import uniffi.place_core.SearchCandidate
import java.io.File

private fun check(cond: Boolean, msg: String, fails: MutableList<String>) {
    if (!cond) fails.add(msg)
}

/** Representative golden assertions, run against any engine. */
private fun runChecks(engine: PlaceEngineInterface, label: String, fails: MutableList<String>) {
    check(engine.rulesetVersion() == "1", "[$label] unexpected ruleset version", fails)

    // Reverse: a peak within 100 m is tagged On.
    engine.reverseGeocode(
        ReverseInput(
            toponyms = listOf(Candidate("Galdhøpiggen", "Fjelltopp", 33.0, "aktiv", null)),
            protectedArea = null,
            address = null,
            kommune = null,
            elevationM = null,
        ),
    ).let { d ->
        check(d?.title == "Galdhøpiggen" && d?.qualifier == Qualifier.ON, "[$label] peak On: $d", fails)
    }

    // Reverse: a containing park wins when no toponym; kommune enriches.
    engine.reverseGeocode(
        ReverseInput(
            toponyms = emptyList(),
            protectedArea = ProtectedArea("Saltfjellet–Svartisen nasjonalpark", "Nasjonalpark"),
            address = null,
            kommune = Kommune("Bodø", "Nordland"),
            elevationM = null,
        ),
    ).let { d ->
        check(
            d?.title == "Saltfjellet–Svartisen nasjonalpark" &&
                d?.qualifier == Qualifier.IN_AREA &&
                d?.kommune == "Bodø",
            "[$label] park wins: $d",
            fails,
        )
    }

    // Reverse: kommune fallback carries no qualifier.
    engine.reverseGeocode(
        ReverseInput(emptyList(), null, null, Kommune("Lom", "Innlandet"), null),
    ).let { d ->
        check(
            d?.title == "Lom" && d?.qualifier == null && d?.secondary == "Innlandet",
            "[$label] kommune fallback: $d",
            fails,
        )
    }

    // Forward: exact beats prefix; icon mapping resolves.
    engine.forwardSearch(
        "stor",
        listOf(
            SearchCandidate("Storsteinnes", "Tettsted", null, null),
            SearchCandidate("Stor", "Fjell", null, null),
        ),
    ).let { hits ->
        check(hits.map { it.title } == listOf("Stor", "Storsteinnes"), "[$label] order: ${hits.map { it.title }}", fails)
        check(hits.first().icon == "mountain", "[$label] icon: ${hits.first().icon}", fails)
    }
}

fun main(args: Array<String>) {
    val fails = mutableListOf<String>()

    // 1. Embedded ruleset.
    runChecks(PlaceEngine.withDefaultRuleset(), "default", fails)

    // 3a. Round-trip: an engine built from the ruleset JSON behaves identically.
    val rulesetPath = args.getOrElse(0) { "ruleset.v1.json" }
    val rulesetJson = File(rulesetPath).readText()
    runChecks(PlaceEngine.fromRulesetJson(rulesetJson), "from-json", fails)

    // 3b. Invalid ruleset raises the typed FFI error rather than crashing.
    try {
        PlaceEngine.fromRulesetJson("{ not valid json")
        fails.add("[error] fromRulesetJson accepted invalid JSON")
    } catch (_: EngineException.InvalidRuleset) {
        // expected
    }

    if (fails.isNotEmpty()) {
        System.err.println("FAIL — ${fails.size} case(s) diverged across the FFI boundary:")
        fails.forEach { System.err.println("  $it") }
        kotlin.system.exitProcess(1)
    }
    println("OK — golden + round-trip + error path pass through the UniFFI Kotlin binding")
}
