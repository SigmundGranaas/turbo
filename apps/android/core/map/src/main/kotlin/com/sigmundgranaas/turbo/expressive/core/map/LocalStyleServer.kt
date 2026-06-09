package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.OverlayId
import com.sigmundgranaas.turbo.expressive.ui.map.MapStyles
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket

/**
 * MapLibre's offline downloader fetches *every* resource — including the style
 * document — through its HTTP file source, which only understands `http(s)`
 * (`file://` and `data:` URIs are rejected). Since our raster styles are generated
 * on-device, this tiny loopback server hands them out over `http://127.0.0.1` so the
 * downloader can read them. It binds to localhost only (never externally reachable)
 * and serves a single GET per base layer: `/<baseId>.json?ov=<Overlay,Overlay>`.
 *
 * The raster *tiles* — base map **and** the requested overlays — are still fetched
 * (and stored) straight from their remote sources, so downloading a region with the
 * avalanche/trail overlay on caches those tiles for offline use too.
 */
internal class LocalStyleServer {

    @Volatile private var server: ServerSocket? = null

    /** Starts the server if needed and returns the style URL for [base] + [overlays]. */
    @Synchronized
    fun styleUrl(base: BaseLayer, overlays: Set<OverlayId> = emptySet()): String {
        val port = ensureStarted()
        val query = if (overlays.isEmpty()) "" else "?ov=" + overlays.joinToString(",") { it.name }
        return "http://127.0.0.1:$port/${base.id}.json$query"
    }

    private fun ensureStarted(): Int {
        server?.let { return it.localPort }
        val socket = ServerSocket(0, BACKLOG, InetAddress.getByName("127.0.0.1"))
        server = socket
        Thread({ acceptLoop(socket) }, "turbo-style-server").apply { isDaemon = true }.start()
        return socket.localPort
    }

    private fun acceptLoop(socket: ServerSocket) {
        while (!socket.isClosed) {
            val client = runCatching { socket.accept() }.getOrNull() ?: break
            runCatching { serve(client) }
            runCatching { client.close() }
        }
    }

    private fun serve(client: Socket) {
        val reader = BufferedReader(InputStreamReader(client.getInputStream()))
        val requestLine = reader.readLine() ?: return
        // "GET /topo.json?ov=Avalanche,Trails HTTP/1.1" → topo + {Avalanche, Trails}
        val target = requestLine.split(" ").getOrNull(1).orEmpty()
        val id = target.substringBefore('?').trimStart('/').substringBefore(".json")
        val base = BaseLayer.entries.firstOrNull { it.id == id }
        val overlays = target.substringAfter("?ov=", "")
            .split(',')
            .mapNotNull { name -> OverlayId.entries.firstOrNull { it.name == name } }
            .toSet()

        val output = client.getOutputStream()
        if (base == null) {
            output.write("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".toByteArray())
            output.flush()
            return
        }
        val body = MapStyles.styleJson(base, overlays).toByteArray()
        val headers = buildString {
            append("HTTP/1.1 200 OK\r\n")
            append("Content-Type: application/json\r\n")
            append("Content-Length: ${body.size}\r\n")
            append("Cache-Control: no-store\r\n")
            append("Connection: close\r\n\r\n")
        }
        output.write(headers.toByteArray())
        output.write(body)
        output.flush()
    }

    private companion object {
        const val BACKLOG = 4
    }
}
