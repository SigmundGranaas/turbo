package com.sigmundgranaas.turbo.expressive.domain

/**
 * Where routing packs are fetched from, asked once per download.
 *
 * The downloader lives in `:core:map` and the setting lives in
 * `:core:data`, which do not depend on each other — the same split
 * [RoutingPack] itself exists to serve. So the two agree on this
 * interface here rather than one reaching into the other.
 *
 * Suspending because the answer comes from DataStore. The alternative —
 * a plain getter over a cached value — would need a scope, a collector
 * and a window in which the cache is stale, all to avoid an await that
 * the caller ([com.sigmundgranaas.turbo.expressive.core.map.PackDownloader.download])
 * is already inside.
 */
fun interface PackSource {
    /** The base URL or `{key}`/`{file}` template to resolve through [RoutingPack.urlFor]. */
    suspend fun current(): String
}
