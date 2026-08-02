package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.RouteSolveRecord
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The last few route solves, in memory, for reading off a phone.
 *
 * Lives in `:core:data` rather than the routing module because both
 * routers write to it and Settings reads it; putting it next to either
 * one would make the other depend on a module it has no other reason to
 * know about.
 *
 * **In memory on purpose.** Persisting it would turn a diagnostic into
 * a record: something that accumulates across sessions, outlives the
 * question it was added for, and has to be reasoned about the next time
 * anyone asks what the app stores. A process death losing the numbers
 * is the correct trade — the tester writes them down, and U3 is what
 * makes them durable and aggregated.
 */
@Singleton
class RouteDiagnostics @Inject constructor() {

    private val _records = MutableStateFlow<List<RouteSolveRecord>>(emptyList())

    /** Most recent first. At most [CAPACITY]. */
    val records: StateFlow<List<RouteSolveRecord>> = _records.asStateFlow()

    fun record(record: RouteSolveRecord) {
        _records.value = (listOf(record) + _records.value).take(CAPACITY)
    }

    fun clear() {
        _records.value = emptyList()
    }

    companion object {
        /**
         * Enough to see a pattern rather than one sample, few enough to
         * read on a phone screen without scrolling past the useful part.
         */
        const val CAPACITY = 20
    }
}
