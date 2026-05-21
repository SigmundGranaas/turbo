using System.Text.Json.Serialization;
using Turbo.Messaging;
using Turboapi.Activities.BackcountrySki.value;

namespace Turboapi.Activities.BackcountrySki.events;

public record BackcountrySkiActivityCreated : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }
    [JsonPropertyName("name")] public string Name { get; init; }
    [JsonPropertyName("description")] public string? Description { get; init; }
    [JsonPropertyName("routeWkt")] public string RouteWkt { get; init; }
    [JsonPropertyName("details")] public BackcountrySkiDetails Details { get; init; }

    [JsonConstructor]
    public BackcountrySkiActivityCreated(
        Guid activityId, Guid ownerId, string name, string? description,
        string routeWkt, BackcountrySkiDetails details)
    {
        ActivityId = activityId;
        OwnerId = ownerId;
        Name = name;
        Description = description;
        RouteWkt = routeWkt;
        Details = details;
    }
}

public record BackcountrySkiActivityUpdated : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }
    [JsonPropertyName("name")] public string? Name { get; init; }
    [JsonPropertyName("description")] public string? Description { get; init; }
    [JsonPropertyName("routeWkt")] public string? RouteWkt { get; init; }
    [JsonPropertyName("details")] public BackcountrySkiDetails? Details { get; init; }
    [JsonPropertyName("version")] public long Version { get; init; }

    [JsonConstructor]
    public BackcountrySkiActivityUpdated(
        Guid activityId, Guid ownerId, string? name, string? description,
        string? routeWkt, BackcountrySkiDetails? details, long version)
    {
        ActivityId = activityId;
        OwnerId = ownerId;
        Name = name;
        Description = description;
        RouteWkt = routeWkt;
        Details = details;
        Version = version;
    }
}

public record BackcountrySkiActivityDeleted : DomainEvent
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("ownerId")] public Guid OwnerId { get; init; }

    [JsonConstructor]
    public BackcountrySkiActivityDeleted(Guid activityId, Guid ownerId)
    {
        ActivityId = activityId;
        OwnerId = ownerId;
    }
}
