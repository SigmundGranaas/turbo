using System.Text.Json.Serialization;
using Turbo.Messaging;
using Turboapi.Activities.Fishing.value;

namespace Turboapi.Activities.Fishing.events;

/// <summary>Fishing activity created. The shared summaries projector
/// consumes the sibling <see cref="value.ActivitySummaryUpserted"/> event
/// to project the cross-kind summary row; the kind-specific projector
/// reads this event to project the typed <c>fishing.activities</c> row.</summary>
public record FishingActivityCreated : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }
    [JsonPropertyName("name")] public string Name { get; init; }
    [JsonPropertyName("description")] public string? Description { get; init; }
    [JsonPropertyName("longitude")] public double Longitude { get; init; }
    [JsonPropertyName("latitude")] public double Latitude { get; init; }
    [JsonPropertyName("details")] public FishingDetails Details { get; init; }

    [JsonConstructor]
    public FishingActivityCreated(
        Guid activityId, Guid ownerId, string name, string? description,
        double longitude, double latitude, FishingDetails details)
    {
        ActivityId = activityId;
        OwnerId = ownerId;
        Name = name;
        Description = description;
        Longitude = longitude;
        Latitude = latitude;
        Details = details;
    }
}

public record FishingActivityUpdated : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }
    [JsonPropertyName("name")] public string? Name { get; init; }
    [JsonPropertyName("description")] public string? Description { get; init; }
    [JsonPropertyName("longitude")] public double? Longitude { get; init; }
    [JsonPropertyName("latitude")] public double? Latitude { get; init; }
    [JsonPropertyName("details")] public FishingDetails? Details { get; init; }
    [JsonPropertyName("version")] public long Version { get; init; }

    [JsonConstructor]
    public FishingActivityUpdated(
        Guid activityId, Guid ownerId, string? name, string? description,
        double? longitude, double? latitude, FishingDetails? details, long version)
    {
        ActivityId = activityId;
        OwnerId = ownerId;
        Name = name;
        Description = description;
        Longitude = longitude;
        Latitude = latitude;
        Details = details;
        Version = version;
    }
}

public record FishingActivityDeleted : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }

    [JsonConstructor]
    public FishingActivityDeleted(Guid activityId, Guid ownerId)
    {
        ActivityId = activityId;
        OwnerId = ownerId;
    }
}
