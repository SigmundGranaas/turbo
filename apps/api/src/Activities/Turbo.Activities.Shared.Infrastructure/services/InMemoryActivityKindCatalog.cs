using Turboapi.Activities.domain.services;

namespace Turboapi.Activities.services;

/// <summary>
/// Default <see cref="IActivityKindCatalog"/>. Wraps the set of
/// <see cref="ActivityKindDescriptor"/>s the host registered in DI; any
/// kind module's extension method that calls
/// <c>services.AddSingleton&lt;ActivityKindDescriptor&gt;(...)</c> shows up
/// here. This is composition: the catalog never knows the kinds by name.
/// </summary>
public sealed class InMemoryActivityKindCatalog : IActivityKindCatalog
{
    private readonly IReadOnlyDictionary<string, ActivityKindDescriptor> _byKey;
    private readonly IReadOnlyList<ActivityKindDescriptor> _all;

    public InMemoryActivityKindCatalog(IEnumerable<ActivityKindDescriptor> descriptors)
    {
        _all = descriptors.OrderBy(d => d.Key, StringComparer.Ordinal).ToList();
        _byKey = _all.ToDictionary(d => d.Key, StringComparer.Ordinal);
    }

    public IReadOnlyList<ActivityKindDescriptor> All() => _all;

    public ActivityKindDescriptor? Get(string kindKey) =>
        _byKey.TryGetValue(kindKey, out var d) ? d : null;
}
