package com.sigmundgranaas.turbo.expressive.core.map

import androidx.compose.runtime.Composable

/**
 * The "one map" composition seams, mirrored from the Flutter app. Each feature
 * contributes layers/overlays/tools to the single shared map instead of editing
 * the map shell — assembled via Hilt `@IntoSet` multibindings as features are
 * modularised (Stage 5).
 */

/** A base/data layer drawn onto the map (markers, route, recording trace, overlays). */
interface MapLayerDescriptor {
    val id: String

    @Composable
    fun Layers()
}

class MapLayerRegistry(val layers: List<MapLayerDescriptor>)

/** Where floating chrome docks. Single-occupant slots show the highest priority. */
enum class MapOverlaySlot { TopCenter, BottomFloating, BottomBar }

/** Persistent floating chrome (selection bar, recording panel, status chip). */
interface MapOverlayDescriptor {
    val id: String
    val slot: MapOverlaySlot
    val priority: Int get() = 0

    @Composable
    fun Content()
}

class MapOverlayRegistry(private val overlays: List<MapOverlayDescriptor>) {
    fun inSlot(slot: MapOverlaySlot): List<MapOverlayDescriptor> =
        overlays.filter { it.slot == slot }.sortedByDescending { it.priority }
}

/** A map tool that can mount layers/overlays and consume taps. One active at a time. */
interface MapToolDescriptor {
    val id: String

    @Composable
    fun Mount()
}

class MapToolRegistry(private val tools: List<MapToolDescriptor>) {
    fun get(id: String): MapToolDescriptor? = tools.firstOrNull { it.id == id }
    val all: List<MapToolDescriptor> get() = tools
}
