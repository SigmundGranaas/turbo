using System.Text.Json.Serialization;
using Turbo.Messaging;
using Turboapi.Activities.Freediving.value;

namespace Turboapi.Activities.Freediving.events;

public record FreedivingActivityCreated : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }
    [JsonPropertyName("name")] public string Name { get; init; }
    [JsonPropertyName("description")] public string? Description { get; init; }
    [JsonPropertyName("longitude")] public double Longitude { get; init; }
    [JsonPropertyName("latitude")] public double Latitude { get; init; }
    [JsonPropertyName("details")] public FreedivingDetails Details { get; init; }

    [JsonConstructor]
    public FreedivingActivityCreated(Guid activityId, Guid ownerId, string name, string? description,
        double longitude, double latitude, FreedivingDetails details)
    { ActivityId = activityId; OwnerId = ownerId; Name = name; Description = description;
      Longitude = longitude; Latitude = latitude; Details = details; }
}

public record FreedivingActivityUpdated : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }
    [JsonPropertyName("name")] public string? Name { get; init; }
    [JsonPropertyName("description")] public string? Description { get; init; }
    [JsonPropertyName("longitude")] public double? Longitude { get; init; }
    [JsonPropertyName("latitude")] public double? Latitude { get; init; }
    [JsonPropertyName("details")] public FreedivingDetails? Details { get; init; }
    [JsonPropertyName("version")] public long Version { get; init; }

    [JsonConstructor]
    public FreedivingActivityUpdated(Guid activityId, Guid ownerId, string? name, string? description,
        double? longitude, double? latitude, FreedivingDetails? details, long version)
    { ActivityId = activityId; OwnerId = ownerId; Name = name; Description = description;
      Longitude = longitude; Latitude = latitude; Details = details; Version = version; }
}

public record FreedivingActivityDeleted : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }

    [JsonConstructor]
    public FreedivingActivityDeleted(Guid activityId, Guid ownerId)
    { ActivityId = activityId; OwnerId = ownerId; }
}
