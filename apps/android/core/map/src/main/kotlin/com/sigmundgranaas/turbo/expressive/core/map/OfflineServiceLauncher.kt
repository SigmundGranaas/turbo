package com.sigmundgranaas.turbo.expressive.core.map

/**
 * Lets the (core:map) tile manager ensure the foreground download service is
 * running without depending on the feature module that hosts it. The
 * implementation lives where the service does and is bound via Hilt; in DEBUG
 * (synthetic manager) there is no service, so nothing injects this.
 */
fun interface OfflineServiceLauncher {
    /** Start/keep the foreground download service alive (idempotent). */
    fun ensureRunning()
}
