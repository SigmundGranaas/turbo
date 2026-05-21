using Turboapi.Activities.BackcountrySki.value;

namespace Turboapi.Activities.BackcountrySki.domain.handler;

public sealed record CreateBackcountrySkiActivityCommand(
    Guid CallerId,
    string Name,
    string? Description,
    string RouteWkt,
    BackcountrySkiDetails Details);

public sealed record UpdateBackcountrySkiActivityCommand(
    Guid CallerId,
    Guid ActivityId,
    string? Name,
    string? Description,
    string? RouteWkt,
    BackcountrySkiDetails? Details)
{
    public long? IfMatchVersion { get; init; }
}

public sealed record DeleteBackcountrySkiActivityCommand(
    Guid CallerId,
    Guid ActivityId)
{
    public long? IfMatchVersion { get; init; }
}
