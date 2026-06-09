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
        // so the service can ship the .so anywhere in its image.
        NativeLibrary.SetDllImportResolver(typeof(PlaceCore).Assembly, (name, asm, path) =>
        {
            if (name != Lib) return IntPtr.Zero;
            var dir = Environment.GetEnvironmentVariable("PLACE_CORE_LIB");
            if (!string.IsNullOrEmpty(dir))
            {
                var full = Path.Combine(dir, "libplace_core.so");
                if (File.Exists(full)) return NativeLibrary.Load(full);
            }
            return IntPtr.Zero; // fall back to the default search
        });
    }

    [DllImport(Lib)]
    private static extern IntPtr place_core_reverse_default(IntPtr inputJsonUtf8);

    [DllImport(Lib)]
    private static extern IntPtr place_core_search_default(IntPtr queryUtf8, IntPtr candidatesJsonUtf8);

    [DllImport(Lib)]
    private static extern void place_core_string_free(IntPtr ptr);

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
