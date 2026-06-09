/*
 * Synthetic UniFFI test client (Kotlin / JVM).
 *
 * Drives `place-core` through the generated Kotlin binding to prove the FFI
 * surface lifts/lowers correctly on the JVM before wiring it into the Android
 * app. Uses a representative subset of the golden cases (constructed via the
 * generated data classes, so no JSON dependency) — full fixture parity is
 * proven by the Python client, which replays every case.
 *
 * NOTE: not executed in the dev container (no kotlinc here). Run where a Kotlin
 * toolchain + JNA exist — see ../README.md for the exact command. When this
 * lands in the Android module it becomes an instrumented/unit test instead.
 */

import uniffi.place_core.Candidate
import uniffi.place_core.Kommune
import uniffi.place_core.PlaceEngine
import uniffi.place_core.ProtectedArea
import uniffi.place_core.Qualifier
import uniffi.place_core.ReverseInput
import uniffi.place_core.SearchCandidate

private fun check(cond: Boolean, msg: String, fails: MutableList<String>) {
    if (!cond) fails.add(msg)
}

fun main() {
    val engine = PlaceEngine.withDefaultRuleset()
    val fails = mutableListOf<String>()

    check(engine.rulesetVersion() == "1", "unexpected ruleset version", fails)

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
        check(d?.title == "Galdhøpiggen" && d?.qualifier == Qualifier.ON, "peak On: $d", fails)
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
            "park wins: $d",
            fails,
        )
    }

    // Reverse: kommune fallback carries no qualifier.
    engine.reverseGeocode(
        ReverseInput(emptyList(), null, null, Kommune("Lom", "Innlandet"), null),
    ).let { d ->
        check(
            d?.title == "Lom" && d?.qualifier == null && d?.secondary == "Innlandet",
            "kommune fallback: $d",
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
        check(hits.map { it.title } == listOf("Stor", "Storsteinnes"), "order: ${hits.map { it.title }}", fails)
        check(hits.first().icon == "mountain", "icon: ${hits.first().icon}", fails)
    }

    if (fails.isNotEmpty()) {
        System.err.println("FAIL — ${fails.size} case(s) diverged across the FFI boundary:")
        fails.forEach { System.err.println("  $it") }
        kotlin.system.exitProcess(1)
    }
    println("OK — golden cases pass through the UniFFI Kotlin binding")
}
