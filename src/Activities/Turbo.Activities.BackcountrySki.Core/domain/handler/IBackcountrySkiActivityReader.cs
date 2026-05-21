namespace Turboapi.Activities.BackcountrySki.domain.handler;

/// <summary>
/// Read-side query port. The infrastructure layer implements this; the
/// command handlers depend on it to fetch the previous state before
/// applying an update or delete.
/// </summary>
public interface IBackcountrySkiActivityReader
{
    Task<BackcountrySkiActivity?> GetByIdAsync(Guid activityId, CancellationToken cancellationToken = default);
}
