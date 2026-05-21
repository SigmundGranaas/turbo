using Turboapi.Activities.BackcountrySki.domain;
using Turboapi.Activities.BackcountrySki.value;

namespace Turboapi.Activities.BackcountrySki.conditions;

/// <summary>
/// Composes weather (+ avalanche in a follow-up phase) into a typed
/// <see cref="BackcountrySkiConditionsReport"/>. The advisor lives in
/// the kind's Core so its scoring rules are part of the kind's
/// behaviour, not a shared base class.
/// </summary>
public interface IBackcountrySkiConditionsAdvisor
{
    Task<BackcountrySkiConditionsReport> AdviseAsync(
        BackcountrySkiActivity activity,
        DateTimeOffset at,
        CancellationToken cancellationToken);
}
