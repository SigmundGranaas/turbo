using System.Text.RegularExpressions;

namespace Turboapi.Places.Ingestion;

/// <summary>
/// Reads the shared ingestion catalog (`infra/k8s/base/ingest/catalog.toml`, mounted via
/// the <c>ingest-catalog</c> ConfigMap) for this service's source parameters.
/// A targeted reader for the catalog's fixed shape (array-of-tables with
/// <c>key = "value"</c> lines) rather than a full TOML dependency — it only
/// needs one field per source, and every caller falls back to a compiled-in
/// constant when the catalog is absent (dev/CI) or the key is missing, so the
/// catalog can never break an ingest.
/// </summary>
public static partial class SourceCatalog
{
    public const string DefaultPath = "/etc/turbo/ingest/catalog.toml";

    [GeneratedRegex(@"^\s*\[\[\s*source\s*\]\]\s*$")]
    private static partial Regex SourceHeader();

    [GeneratedRegex("""^\s*(?<key>[A-Za-z_]+)\s*=\s*"(?<val>[^"]*)"\s*$""")]
    private static partial Regex KeyValue();

    /// <summary>The value of <paramref name="key"/> in the <c>[[source]]</c>
    /// table whose <c>id</c> equals <paramref name="id"/>, or null if absent.
    /// Pure (operates on the file text) so it is unit-testable.</summary>
    public static string? TryGetValue(string catalogToml, string id, string key)
    {
        string? currentId = null;
        var fields = new Dictionary<string, string>(StringComparer.Ordinal);
        var blocks = new List<(string? Id, Dictionary<string, string> Fields)>();

        foreach (var raw in catalogToml.Split('\n'))
        {
            var line = raw.TrimEnd('\r');
            if (SourceHeader().IsMatch(line))
            {
                blocks.Add((currentId, fields));
                currentId = null;
                fields = new Dictionary<string, string>(StringComparer.Ordinal);
                continue;
            }
            var m = KeyValue().Match(line);
            if (!m.Success) continue;
            var k = m.Groups["key"].Value;
            var v = m.Groups["val"].Value;
            if (k == "id") currentId = v;
            fields[k] = v;
        }
        blocks.Add((currentId, fields));

        foreach (var (blockId, blockFields) in blocks)
        {
            if (blockId == id && blockFields.TryGetValue(key, out var value))
                return value;
        }
        return null;
    }

    /// <summary>Reads <paramref name="key"/> for source <paramref name="id"/>
    /// from the mounted catalog (path via <c>INGEST_CATALOG_PATH</c>), or null if
    /// the file is absent/unreadable/missing the key.</summary>
    public static string? TryReadValue(string id, string key)
    {
        var path = Environment.GetEnvironmentVariable("INGEST_CATALOG_PATH") ?? DefaultPath;
        try
        {
            return File.Exists(path) ? TryGetValue(File.ReadAllText(path), id, key) : null;
        }
        catch (IOException)
        {
            return null;
        }
    }
}
