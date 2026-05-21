using System.Text.Json.Serialization;
using Turbo.Messaging;
using Turboapi.Activities.value;

namespace Turboapi.Activities.events;

/// <summary>
/// Emitted by every kind's write-side handler after a successful create or
/// update. The shared summaries projector consumes it and upserts a row in
/// <c>shared.activity_summaries</c>. Carries enough rendering hints
/// (iconKey, colorHex) that the read endpoint never has to look at kind
/// tables to paint a map.
/// </summary>
public record ActivitySummaryUpserted : DomainEvent
{
    [JsonPropertyName("activityId")]
    public Guid ActivityId { get; init; }

    [JsonPropertyName("ownerId")]
    public Guid OwnerId { get; init; }

    [JsonPropertyName("kind")]
    public string Kind { get; init; }

    [JsonPropertyName("name")]
    public string Name { get; init; }

    [JsonPropertyName("geometry")]
    public ActivityGeometryWkt Geometry { get; init; }

    [JsonPropertyName("iconKey")]
    public string IconKey { get; init; }

    [JsonPropertyName("colorHex")]
    public string? ColorHex { get; init; }

    [JsonPropertyName("version")]
    public long Version { get; init; }

    [JsonConstructor]
    public ActivitySummaryUpserted(
        Guid activityId,
        Guid ownerId,
        string kind,
        string name,
        ActivityGeometryWkt geometry,
        string iconKey,
        string? colorHex,
        long version)
    {
        ActivityId = activityId;
        OwnerId = ownerId;
        Kind = kind;
        Name = name;
        Geometry = geometry;
        IconKey = iconKey;
        ColorHex = colorHex;
        Version = version;
    }
}

/// <summary>
/// Emitted by a kind's delete handler. The summaries projector
/// tombstones the row (sets <c>deleted_at</c>) so the delta-sync
/// endpoint can surface it to clients.
/// </summary>
public record ActivitySummaryDeleted : DomainEvent
{
    [JsonPropertyName("activityId")]
    public Guid ActivityId { get; init; }

    [JsonPropertyName("ownerId")]
    public Guid OwnerId { get; init; }

    [JsonPropertyName("kind")]
    public string Kind { get; init; }

    [JsonConstructor]
    public ActivitySummaryDeleted(Guid activityId, Guid ownerId, string kind)
    {
        ActivityId = activityId;
        OwnerId = ownerId;
        Kind = kind;
    }
}
