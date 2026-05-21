using System.Text.Json.Serialization;

namespace Turboapi.Activities.value;

/// <summary>
/// Wire-format geometry payload carried on summary events. WKT (well-known
/// text) is used so the event payload is self-describing and human-readable
/// in outbox JSON, at the cost of slightly larger byte size vs WKB. Always
/// in EPSG:4326.
/// </summary>
public sealed record ActivityGeometryWkt
{
    [JsonPropertyName("kind")]
    public ActivityGeometryKind Kind { get; init; }

    [JsonPropertyName("wkt")]
    public string Wkt { get; init; }

    [JsonConstructor]
    public ActivityGeometryWkt(ActivityGeometryKind kind, string wkt)
    {
        Kind = kind;
        Wkt = wkt ?? throw new ArgumentNullException(nameof(wkt));
    }
}
