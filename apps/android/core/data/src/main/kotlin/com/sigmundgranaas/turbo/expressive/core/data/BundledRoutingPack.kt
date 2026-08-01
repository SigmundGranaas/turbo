package com.sigmundgranaas.turbo.expressive.core.data

/**
 * A routing pack shipped inside the APK, installable without a network.
 *
 * # Why this exists
 *
 * Normally a pack is cut on demand by the tileserver and downloaded with
 * the map tiles. That is the right design and it is what ships. But it
 * makes the phone's routing engine untestable whenever the server is
 * not there — and the first measurement of that engine on real hardware
 * has to happen *somewhere*, on a real APK, before anyone can argue
 * about whether the device path should lead.
 *
 * So one real region rides along in the APK. The pack format is a
 * directory of files with a manifest; nothing about it requires a
 * server, and `PackStore` will find it the moment it is on disk in the
 * right place. The server builds packs; it does not own them.
 *
 * # Not installed automatically
 *
 * It costs tens of megabytes of the user's storage, and most users will
 * never route in this one region. An explicit action, with the size on
 * it, is the honest way to spend someone else's disk.
 */
interface BundledRoutingPack {

    /** The pack key, which is also the directory it installs into. */
    val key: String

    /** Uncompressed size on disk after installing. */
    val sizeBytes: Long

    /** Human-readable region description, for the settings row. */
    val description: String

    /** Is it already unpacked into the pack store? */
    fun isInstalled(): Boolean

    /**
     * Copy it out of the APK's assets into the pack store.
     *
     * Assembled beside the destination and moved into place, so a
     * process death mid-copy cannot leave a half-written DEM that opens
     * successfully and reports missing ground as untraversable — the
     * same failure `PackDownloader` guards against, for the same reason.
     */
    suspend fun install(): Result<Unit>

    /** Remove it, freeing [sizeBytes]. */
    suspend fun uninstall()
}
