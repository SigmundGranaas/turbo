using Turboapi.Activities.Fishing.domain;
using Turboapi.Activities.Fishing.value;

namespace Turboapi.Activities.Fishing.conditions;

/// <summary>
/// Composes the weather provider (and, in follow-ups, tides + river
/// flow) into a typed <see cref="FishingConditionsReport"/> for one
/// activity at one instant. Kind-specific scoring lives here — not in
/// the aggregate, not in a shared base.
/// </summary>
public interface IFishingConditionsAdvisor
{
    Task<FishingConditionsReport> AdviseAsync(
        FishingActivity activity,
        DateTimeOffset at,
        CancellationToken cancellationToken);
}
