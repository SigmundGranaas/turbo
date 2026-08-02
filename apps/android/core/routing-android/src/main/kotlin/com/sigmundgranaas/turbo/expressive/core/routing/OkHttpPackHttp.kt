package com.sigmundgranaas.turbo.expressive.core.routing

import okhttp3.OkHttpClient
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import uniffi.turbo_route_ffi.HttpResponse
import uniffi.turbo_route_ffi.PackHttp
import uniffi.turbo_route_ffi.RouteException
import java.util.concurrent.TimeUnit

/**
 * The app's HTTP client, lent to the pack builder.
 *
 * The Rust side ships no HTTP client at all. That is deliberate and
 * measured: linking `reqwest` + `rustls` + `tokio` into the native
 * library cost **3.2 MB per ABI**, roughly tripling it, to duplicate a
 * TLS stack and an async runtime the app already had. Passing OkHttp
 * across the FFI instead brought that to 0.86 MB — and the builder now
 * inherits this client's connection pool, proxy handling and trust
 * configuration rather than having its own opinion about all three.
 *
 * Called from Rust worker threads, several at a time. OkHttp is
 * thread-safe and pools connections across them, which is exactly the
 * behaviour wanted here.
 */
class OkHttpPackHttp(
    /**
     * Share the app's client rather than building one, so this rides on
     * the same pool and configuration as every other request. A fresh
     * `OkHttpClient` per caller is the documented way to waste
     * connections.
     */
    base: OkHttpClient,
) : PackHttp {

    /**
     * The same client with a much longer read timeout.
     *
     * `newBuilder` shares the pool and dispatcher, so this is not a
     * second client. The timeout has to move: a cold WCS coverage
     * genuinely takes minutes, and OkHttp's default 10 s read timeout
     * would fail a region build most of the way through — the exact
     * failure that is most expensive to hit.
     */
    private val client: OkHttpClient = base.newBuilder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(READ_TIMEOUT_MIN, TimeUnit.MINUTES)
        .writeTimeout(60, TimeUnit.SECONDS)
        .callTimeout(READ_TIMEOUT_MIN, TimeUnit.MINUTES)
        .build()

    override fun get(url: String): HttpResponse = execute(
        Request.Builder().url(url).header("user-agent", UA).get().build(),
    )

    override fun postJson(url: String, body: String): HttpResponse = execute(
        Request.Builder()
            .url(url)
            .header("user-agent", UA)
            .post(body.toRequestBody(JSON))
            .build(),
    )

    /**
     * Run one request and hand back status *and* body.
     *
     * A non-2xx is returned, not thrown. The builder quotes both in its
     * diagnostics — a WCS 400 carries an XML explanation of what was
     * wrong with the request — and throwing here would replace that
     * with "HTTP 400" and nothing else.
     *
     * Only a transport failure becomes an exception, because there is
     * no response to describe.
     */
    private fun execute(request: Request): HttpResponse = try {
        client.newCall(request).execute().use { response ->
            HttpResponse(
                status = response.code.toUShort(),
                // `bytes()` and not `string()`: most of what comes back
                // here is a GeoTIFF or a zip, and decoding those as
                // UTF-8 would corrupt them silently.
                body = response.body?.bytes() ?: ByteArray(0),
            )
        }
    } catch (e: Exception) {
        throw RouteException.Pack("${request.url.host}: ${e.message ?: e::class.java.simpleName}")
    }

    private companion object {
        val JSON = "application/json".toMediaType()
        const val UA = "turbo-android-pack-build"

        /**
         * Long, because the WCS is. Bounded anyway so a dead connection
         * cannot hold a build open forever.
         */
        const val READ_TIMEOUT_MIN = 5L
    }
}
