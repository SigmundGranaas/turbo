using System.Text.Json.Serialization;
using Turbo.Messaging;
using Turboapi.Activities.Packrafting.value;

namespace Turboapi.Activities.Packrafting.events;

public record PackraftingActivityCreated : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }
    [JsonPropertyName("name")] public string Name { get; init; }
    [JsonPropertyName("description")] public string? Description { get; init; }
    [JsonPropertyName("routeWkt")] public string RouteWkt { get; init; }
    [JsonPropertyName("details")] public PackraftingDetails Details { get; init; }

    [JsonConstructor]
    public PackraftingActivityCreated(Guid activityId, Guid ownerId, string name, string? description, string routeWkt, PackraftingDetails details)
    { ActivityId = activityId; OwnerId = ownerId; Name = name; Description = description; RouteWkt = routeWkt; Details = details; }
}

public record PackraftingActivityUpdated : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }
    [JsonPropertyName("name")] public string? Name { get; init; }
    [JsonPropertyName("description")] public string? Description { get; init; }
    [JsonPropertyName("routeWkt")] public string? RouteWkt { get; init; }
    [JsonPropertyName("details")] public PackraftingDetails? Details { get; init; }
    [JsonPropertyName("version")] public long Version { get; init; }

    [JsonConstructor]
    public PackraftingActivityUpdated(Guid activityId, Guid ownerId, string? name, string? description, string? routeWkt, PackraftingDetails? details, long version)
    { ActivityId = activityId; OwnerId = ownerId; Name = name; Description = description; RouteWkt = routeWkt; Details = details; Version = version; }
}

public record PackraftingActivityDeleted : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }

    [JsonConstructor]
    public PackraftingActivityDeleted(Guid activityId, Guid ownerId)
    { ActivityId = activityId; OwnerId = ownerId; }
}
