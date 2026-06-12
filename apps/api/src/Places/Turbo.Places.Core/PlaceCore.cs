using System.Runtime.InteropServices;

namespace Turboapi.Places.Core;

/// <summary>
/// P/Invoke wrapper over <c>libplace_core</c> (the shared Rust ranking core,
/// built with <c>--features cabi</c>). JSON in / JSON out — the server already
/// speaks JSON everywhere, so this sidesteps struct marshalling. The native
/// library is resolved by name; set the loader path (e.g. <c>LD_LIBRARY_PATH</c>)
/// or the <c>PLACE_CORE_LIB</c> env var to its directory.
/// </summary>
public static class PlaceCore
{
    private const string Lib = "place_core";

    static PlaceCore()
    {
        // Let an explicit PLACE_CORE_LIB dir win over the default loader search,
        // so the service can ship the lib anywhere in its image — and resolve
        // the right filename per OS (Linux .so / macOS .dylib / Windows .dll).
        NativeLibrary.SetDllImportResolver(typeof(PlaceCore).Assembly, (name, asm, path) =>
        {
            if (name != Lib) return IntPtr.Zero;
            var dir = Environment.GetEnvironmentVariable("PLACE_CORE_LIB");
            if (!string.IsNullOrEmpty(dir))
            {
                foreach (var candidate in new[] { "libplace_core.so", "libplace_core.dylib", "place_core.dll" })
                {
                    var full = Path.Combine(dir, candidate);
                    if (File.Exists(full)) return NativeLibrary.Load(full);
                }
            }
            return IntPtr.Zero; // fall back to the default OS search
        });
    }

    [DllImport(Lib)]
    private static extern IntPtr place_core_reverse_default(IntPtr inputJsonUtf8);

    [DllImport(Lib)]
    private static extern IntPtr place_core_search_default(IntPtr queryUtf8, IntPtr candidatesJsonUtf8);

    [DllImport(Lib)]
    private static extern IntPtr place_core_ruleset_default();

    [DllImport(Lib)]
    private static extern void place_core_string_free(IntPtr ptr);

    [DllImport(Lib)]
    private static extern IntPtr place_core_bundle_open(IntPtr pathUtf8);

    [DllImport(Lib)]
    private static extern void place_core_bundle_free(IntPtr bundle);

    [DllImport(Lib)]
    private static extern IntPtr place_core_bundle_reverse(IntPtr bundle, double lat, double lng);

    /// <summary>Open an offline bundle file; returns a handle (zero on error)
    /// to free with <see cref="BundleFree"/>. Requires the lib built with
    /// <c>--features cabi,embedded</c>.</summary>
    public static IntPtr BundleOpen(string path)
    {
        var p = Marshal.StringToCoTaskMemUTF8(path);
        try { return place_core_bundle_open(p); }
        finally { Marshal.FreeCoTaskMem(p); }
    }

    public static void BundleFree(IntPtr bundle) => place_core_bundle_free(bundle);

    /// <summary>Reverse-geocode from an opened bundle → JSON
    /// <c>LocationDescription</c> (or <c>null</c>).</summary>
    public static string BundleReverseJson(IntPtr bundle, double lat, double lng)
    {
        var result = place_core_bundle_reverse(bundle, lat, lng);
        try { return Marshal.PtrToStringUTF8(result) ?? "null"; }
        finally { place_core_string_free(result); }
    }

    /// <summary>The embedded ruleset artifact (verbatim JSON) — what the core
    /// runs, so the server serves exactly that.</summary>
    public static string RulesetJson()
    {
        var result = place_core_ruleset_default();
        try { return Marshal.PtrToStringUTF8(result) ?? "{}"; }
        finally { place_core_string_free(result); }
    }

    /// <summary>Reverse-geocode a JSON <c>ReverseInput</c> → JSON
    /// <c>LocationDescription</c> (or the literal <c>null</c>).</summary>
    public static string ReverseJson(string inputJson)
    {
        var input = Marshal.StringToCoTaskMemUTF8(inputJson);
        try
        {
            var result = place_core_reverse_default(input);
            try { return Marshal.PtrToStringUTF8(result) ?? "null"; }
            finally { place_core_string_free(result); }
        }
        finally { Marshal.FreeCoTaskMem(input); }
    }

    /// <summary>Rank a JSON array of <c>SearchCandidate</c> for <paramref name="query"/>
    /// → JSON array of <c>SearchHit</c>.</summary>
    public static string SearchJson(string query, string candidatesJson)
    {
        var q = Marshal.StringToCoTaskMemUTF8(query);
        var c = Marshal.StringToCoTaskMemUTF8(candidatesJson);
        try
        {
            var result = place_core_search_default(q, c);
            try { return Marshal.PtrToStringUTF8(result) ?? "[]"; }
            finally { place_core_string_free(result); }
        }
        finally
        {
            Marshal.FreeCoTaskMem(q);
            Marshal.FreeCoTaskMem(c);
        }
    }
}
