namespace Turboapi.Activities.Hiking.domain.handler;

public interface IHikingActivityReader
{
    Task<HikingActivity?> GetByIdAsync(Guid activityId, CancellationToken cancellationToken = default);
}
