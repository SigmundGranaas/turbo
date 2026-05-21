using Turboapi.Activities.value;

namespace Turboapi.Activities.domain.services;

/// <summary>
/// Server-side registry of which kinds are enabled in this deployment.
/// The shared discovery endpoint returns these to clients so a release that
/// adds a new kind to the server doesn't require a coordinated client
/// release — clients ignore unknown keys, and the picker surfaces the new
/// kind for clients that have shipped its UI bundle.
///
/// Each kind module contributes a single <see cref="ActivityKindDescriptor"/>
/// via the DI container at startup; the catalog is a thin facade over those.
/// </summary>
public interface IActivityKindCatalog
{
    IReadOnlyList<ActivityKindDescriptor> All();

    ActivityKindDescriptor? Get(string kindKey);
}

public sealed record ActivityKindDescriptor
{
    public required string Key { get; init; }
    public required string DisplayName { get; init; }
    public required string IconKey { get; init; }
    public required string ColorHex { get; init; }
    public required IReadOnlySet<ActivityGeometryKind> AllowedGeometries { get; init; }
    public required bool ConditionsAvailable { get; init; }
}
