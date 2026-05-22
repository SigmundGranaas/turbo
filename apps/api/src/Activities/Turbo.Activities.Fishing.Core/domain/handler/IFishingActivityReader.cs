using Turboapi.Activities.Fishing.domain;

namespace Turboapi.Activities.Fishing.domain.handler;

/// <summary>
/// Read-side query port. Defined in Core so handlers can reach the read
/// model without taking an EF Core dependency; the Infrastructure layer
/// implements it.
/// </summary>
public interface IFishingActivityReader
{
    Task<FishingActivity?> GetByIdAsync(Guid activityId, CancellationToken cancellationToken = default);
}
