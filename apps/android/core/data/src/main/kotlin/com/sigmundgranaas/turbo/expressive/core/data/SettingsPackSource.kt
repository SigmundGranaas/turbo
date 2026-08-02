package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.PackSource
import com.sigmundgranaas.turbo.expressive.domain.RoutingPack
import kotlinx.coroutines.flow.first
import javax.inject.Inject
import javax.inject.Singleton

/**
 * [PackSource] backed by the user's setting, falling back to
 * [RoutingPack.DEFAULT_SOURCE].
 *
 * `first()` and not a held value: this is read once per pack download —
 * a multi-megabyte operation a user starts by hand — so the cost of
 * reading DataStore is not measurable, and reading it fresh is what
 * makes a corrected URL take effect on the next retry rather than the
 * next launch.
 */
@Singleton
class SettingsPackSource @Inject constructor(
    private val settings: SettingsRepository,
) : PackSource {
    override suspend fun current(): String =
        settings.settings.first().packSourceUrl ?: RoutingPack.DEFAULT_SOURCE
}
