# MapLibre uses native + reflection; keep its classes.
-keep class org.maplibre.android.** { *; }
-keep class com.mapbox.** { *; }
-dontwarn org.maplibre.android.**

# ── JNA + uniffi: the routing engine's Kotlin/Rust boundary ────────────────
#
# Release-only breakage lives here, which is why these rules exist before
# anyone has seen them fail. The debug build does not minify, so every
# on-device route run so far has been against un-minified bindings.
#
# JNA does not call these classes, it REFLECTS over them:
#
#   @Structure.FieldOrder("capacity", "len", "data")
#   open class RustBuffer : Structure() { @JvmField var capacity: Long ... }
#
# The field order is a list of *strings* matched against real field names
# at runtime. R8 renames fields to `a`, `b`, `c` and leaves the strings
# alone, so JNA looks for "capacity" on a class that no longer has it and
# throws — after the APK is signed, published, and installed. Keeping the
# members is not optional, and shrinking cannot be made clever enough to
# see the dependency.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.Structure {
    <fields>;
    *;
}
# Callback interfaces are invoked from Rust through a function pointer;
# nothing in Kotlin calls them, so R8 sees them as dead code.
-keep class * implements com.sun.jna.Callback { *; }
-keepclassmembers class * implements com.sun.jna.Callback { *; }
# JNA ships desktop AWT paths it never takes on Android.
-dontwarn java.awt.**

# The generated bindings themselves. `uniffi.*` is generated code that
# nothing references by name — the app talks to a hand-written wrapper,
# which talks to these — so R8 has every reason to strip them and no way
# to know Rust holds the other end.
-keep class uniffi.** { *; }
-keepclassmembers class uniffi.** { *; }
