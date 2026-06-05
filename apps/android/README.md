# Turbo · Android (Material 3 Expressive)

A native **Jetpack Compose** reimagining of the Turbo hiking-map app in the
**Material 3 Expressive** language, built from the `Turbo · Material 3 Expressive`
design bundle. Warm-rust color scheme, rounder shapes, emphasized type, springy
motion, and the expressive component family (FAB speed-dial, connected button
groups, wavy progress, the morphing loading indicator, cookie/blob activity
glyphs) over a full-bleed real map.

This is a separate app from the Flutter build at `apps/flutter`
(`applicationId = com.sigmundgranaas.turbo.expressive`, so both can be installed
side by side).

## Stack

- Jetpack Compose + **Material 3 Expressive** (`androidx.compose.material3:1.5.0-alpha`)
- Kotlin 2.3.20 · AGP 9.2.1 · Gradle 9.4.1 · compileSdk 37 · minSdk 26
- **MapLibre GL Native** (`org.maplibre.gl:android-sdk`) for real Kartverket /
  Norgeskart + OSM + satellite raster tiles; markers & routes are drawn as a
  Compose overlay projected onto the live camera.
- Navigation-Compose; lean architecture (ViewModel + StateFlow, no DI framework).

## Build & run

```bash
cd apps/android
./gradlew :app:assembleDebug      # build the debug APK
./gradlew :app:lintDebug          # static analysis

# install on a connected device / emulator:
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

The Gradle wrapper is committed; the SDK path is read from `local.properties`
(`sdk.dir`). Open the `apps/android` folder directly in Android Studio to use the
`@Preview`s (each screen/component previews in light + dark).

## Layout

```
app/src/main/kotlin/com/sigmundgranaas/turbo/expressive/
  MainActivity.kt              entry; hosts TurboTheme + nav graph
  ui/theme/                    Color · Shape · Type · Theme (MaterialExpressiveTheme)
  ui/components/               Cookie · MarkerPin · SearchPill · MapControlRail ·
                               MapFabMenu · Primitives (rows, section labels)
  ui/map/                      TurboMap (MapLibre AndroidView + overlay) · MapStyles
  ui/nav/                      TurboNavGraph
  domain/                      Models · SampleData (Tromsø / Lyngen seed)
  feature/{map,search,markers,layers,nav,settings,recording,activity}/
```

## Scope (v1)

Foundation + navigable core: Map home (search, control rail, FAB speed-dial,
following), Search, Marker info & New marker sheets, Map layers sheet, Nav
drawer, Settings, Recording (live wavy progress + docked-FAB bar), and a
Backcountry-ski activity detail (segmented tabs, avalanche verdict, aspect rose,
elevation profile). Remaining design frames (paths, more activities, weather,
overlays, offline, dialogs) slot onto the same kit.
