using System.Text.RegularExpressions;

namespace Turboapi.Places.Ingestion;

/// <summary>
/// Canonicalisation rules shared by every ingestion path (REST sampling, GPKG
/// bulk) so they emit identical <c>Place</c> rows. Mirrors the live app: reject
/// placeholder names and bare Naturbase codes; fold names for trigram/FTS while
/// keeping Norwegian letters (conflating ø→o would hurt search precision).
/// </summary>
public static partial class Normalization
{
    private static readonly HashSet<string> Placeholders =
        new(StringComparer.OrdinalIgnoreCase) { "ukjent", "unknown" };

    /// <summary>A Naturbase area code like "VV00002858" / "VR123" — must never
    /// surface as a name.</summary>
    [GeneratedRegex(@"^[A-Z]{1,5}\d+$")]
    private static partial Regex NaturbaseCode();

    public static bool IsUsableName(string? name)
    {
        if (string.IsNullOrWhiteSpace(name)) return false;
        var trimmed = name.Trim();
        if (Placeholders.Contains(trimmed)) return false;
        if (NaturbaseCode().IsMatch(trimmed)) return false;
        return true;
    }

    /// <summary>Lower-cased fold for trigram/FTS, preserving æ/ø/å.</summary>
    public static string Fold(string name) => name.Trim().ToLowerInvariant();
}
