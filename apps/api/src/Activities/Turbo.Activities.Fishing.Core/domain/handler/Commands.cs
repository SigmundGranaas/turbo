using Turboapi.Activities.Fishing.value;

namespace Turboapi.Activities.Fishing.domain.handler;

public sealed record CreateFishingActivityCommand(
    Guid CallerId,
    string Name,
    string? Description,
    double Longitude,
    double Latitude,
    FishingDetails Details);

public sealed record UpdateFishingActivityCommand(
    Guid CallerId,
    Guid ActivityId,
    string? Name,
    string? Description,
    double? Longitude,
    double? Latitude,
    FishingDetails? Details)
{
    /// <summary>
    /// Optional optimistic-concurrency check. When set, the handler
    /// rejects the command (with <see cref="domain.exception.OptimisticConcurrencyException"/>)
    /// if the stored aggregate's version does not match. Matches the
    /// Tracks update flow's If-Match contract.
    /// </summary>
    public long? IfMatchVersion { get; init; }
}

public sealed record DeleteFishingActivityCommand(
    Guid CallerId,
    Guid ActivityId)
{
    public long? IfMatchVersion { get; init; }
}
