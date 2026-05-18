namespace Turboapi.Activity.domain.query;

public class ActivityQuery
{
    public record GetActivityByIdQuery(Guid ActivityId, Guid UserId);
}