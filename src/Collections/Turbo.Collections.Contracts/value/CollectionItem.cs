using System.Text.Json.Serialization;

namespace Turboapi.Collections.domain.value;

/// <summary>
/// A polymorphic reference to an item that lives in a collection. The
/// server stores (type, uuid) opaquely — it doesn't dereference into
/// the source module to verify the item still exists, which keeps
/// Collections free of cross-module coupling.
/// </summary>
public record CollectionItemRef
{
    public const string TypeMarker = "marker";
    public const string TypePath = "path";

    [JsonPropertyName("type")]
    public string Type { get; init; }

    [JsonPropertyName("uuid")]
    public string Uuid { get; init; }

    [JsonConstructor]
    public CollectionItemRef(string type, string uuid)
    {
        Type = type;
        Uuid = uuid;
    }

    public CollectionItemRef()
    {
        Type = string.Empty;
        Uuid = string.Empty;
    }
}
