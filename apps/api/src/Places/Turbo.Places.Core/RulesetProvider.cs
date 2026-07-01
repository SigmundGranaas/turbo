using System.Text.Json;

namespace Turboapi.Places.Core;

/// <summary>
/// Serves the place-core ruleset artifact (for <c>GET /api/places/ruleset/
/// {version}</c> and, later, bundle embedding). Reads it once from the native
/// core so server and core never drift.
/// </summary>
public sealed class RulesetProvider
{
    public string Json { get; }
    public string Version { get; }

    /// <summary>Lowercased feature-type → metres-of-head-start prominence bonus
    /// (<c>kind_prominence × prominence_weight</c>), the same product the native
    /// reranker computes — so the DB retrieval ordering and the final rank agree
    /// on prominence. Empty when the ruleset carries no prominence table.</summary>
    public IReadOnlyDictionary<string, double> KindBonusMeters { get; }

    /// <summary>Bonus for feature types absent from <see cref="KindBonusMeters"/>
    /// (<c>prominence_default × prominence_weight</c>).</summary>
    public double DefaultBonusMeters { get; }

    public RulesetProvider()
    {
        Json = PlaceCore.RulesetJson();
        using var doc = JsonDocument.Parse(Json);
        var root = doc.RootElement;
        Version = root.TryGetProperty("version", out var v)
            ? v.GetString() ?? "unknown"
            : "unknown";

        var weight = root.TryGetProperty("prominence_weight", out var w) ? w.GetDouble() : 0.0;
        var promDefault = root.TryGetProperty("prominence_default", out var pd) ? pd.GetDouble() : 0.0;
        DefaultBonusMeters = promDefault * weight;

        var map = new Dictionary<string, double>();
        if (root.TryGetProperty("kind_prominence", out var kp) && kp.ValueKind == JsonValueKind.Object)
        {
            foreach (var prop in kp.EnumerateObject())
                map[prop.Name.ToLowerInvariant()] = prop.Value.GetDouble() * weight;
        }
        KindBonusMeters = map;
    }

    /// <summary>The ruleset JSON for <paramref name="version"/>, or null if we
    /// don't have that version. (Only the embedded version exists today;
    /// ingested versions join when ruleset evolution lands.)</summary>
    public string? ForVersion(string version) =>
        version == Version ? Json : null;
}
