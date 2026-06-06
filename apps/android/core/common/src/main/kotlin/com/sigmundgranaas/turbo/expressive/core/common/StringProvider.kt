package com.sigmundgranaas.turbo.expressive.core.common

/**
 * Resolves Android string resources from non-UI layers (ViewModels, mappers)
 * without depending on a `Context` directly — keeps those layers testable with a
 * fake and keeps `:core:common` free of the Android SDK. The [id] is an
 * `@StringRes` resource identifier; the Android implementation lives in
 * `:core:data` (`AndroidStringProvider`).
 */
interface StringProvider {
    fun get(id: Int): String
    fun get(id: Int, vararg formatArgs: Any): String
}
