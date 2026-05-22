using System.Text.Json.Serialization;
using Turbo.Messaging;
using Turboapi.Activities.XcSki.value;

namespace Turboapi.Activities.XcSki.events;

public record XcSkiActivityCreated : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }
    [JsonPropertyName("name")] public string Name { get; init; }
    [JsonPropertyName("description")] public string? Description { get; init; }
    [JsonPropertyName("routeWkt")] public string RouteWkt { get; init; }
    [JsonPropertyName("details")] public XcSkiDetails Details { get; init; }

    [JsonConstructor]
    public XcSkiActivityCreated(Guid activityId, Guid ownerId, string name, string? description, string routeWkt, XcSkiDetails details)
    { ActivityId = activityId; OwnerId = ownerId; Name = name; Description = description; RouteWkt = routeWkt; Details = details; }
}

public record XcSkiActivityUpdated : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }
    [JsonPropertyName("name")] public string? Name { get; init; }
    [JsonPropertyName("description")] public string? Description { get; init; }
    [JsonPropertyName("routeWkt")] public string? RouteWkt { get; init; }
    [JsonPropertyName("details")] public XcSkiDetails? Details { get; init; }
    [JsonPropertyName("version")] public long Version { get; init; }

    [JsonConstructor]
    public XcSkiActivityUpdated(Guid activityId, Guid ownerId, string? name, string? description, string? routeWkt, XcSkiDetails? details, long version)
    { ActivityId = activityId; OwnerId = ownerId; Name = name; Description = description; RouteWkt = routeWkt; Details = details; Version = version; }
}

public record XcSkiActivityDeleted : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }

    [JsonConstructor]
    public XcSkiActivityDeleted(Guid activityId, Guid ownerId)
    { ActivityId = activityId; OwnerId = ownerId; }
}
