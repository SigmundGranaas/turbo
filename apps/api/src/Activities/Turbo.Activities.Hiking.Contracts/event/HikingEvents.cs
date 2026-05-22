using System.Text.Json.Serialization;
using Turbo.Messaging;
using Turboapi.Activities.Hiking.value;

namespace Turboapi.Activities.Hiking.events;

public record HikingActivityCreated : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }
    [JsonPropertyName("name")] public string Name { get; init; }
    [JsonPropertyName("description")] public string? Description { get; init; }
    [JsonPropertyName("routeWkt")] public string RouteWkt { get; init; }
    [JsonPropertyName("details")] public HikingDetails Details { get; init; }

    [JsonConstructor]
    public HikingActivityCreated(Guid activityId, Guid ownerId, string name, string? description, string routeWkt, HikingDetails details)
    { ActivityId = activityId; OwnerId = ownerId; Name = name; Description = description; RouteWkt = routeWkt; Details = details; }
}

public record HikingActivityUpdated : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }
    [JsonPropertyName("name")] public string? Name { get; init; }
    [JsonPropertyName("description")] public string? Description { get; init; }
    [JsonPropertyName("routeWkt")] public string? RouteWkt { get; init; }
    [JsonPropertyName("details")] public HikingDetails? Details { get; init; }
    [JsonPropertyName("version")] public long Version { get; init; }

    [JsonConstructor]
    public HikingActivityUpdated(Guid activityId, Guid ownerId, string? name, string? description, string? routeWkt, HikingDetails? details, long version)
    { ActivityId = activityId; OwnerId = ownerId; Name = name; Description = description; RouteWkt = routeWkt; Details = details; Version = version; }
}

public record HikingActivityDeleted : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }

    [JsonConstructor]
    public HikingActivityDeleted(Guid activityId, Guid ownerId)
    { ActivityId = activityId; OwnerId = ownerId; }
}
