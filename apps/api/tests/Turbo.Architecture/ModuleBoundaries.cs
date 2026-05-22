using System.Reflection;
using FluentAssertions;
using Xunit;

namespace Turbo.Architecture;

/// <summary>
/// The four modules must not have direct references to one another. If
/// Tracks ever depended on Geo (or vice versa) the "messages only"
/// contract would silently degrade to "messages plus a sneaky compile-time
/// call." The shared abstraction packages (Turbo.Messaging.*,
/// Turbo.Outbox.*, Turbo.Messaging.Nats / InProcess) are the only legal way
/// for one module to reach another.
/// </summary>
public sealed class ModuleBoundaries
{
    private static readonly string[] TracksAssemblies =
    [
        "Turbo.Tracks.Core",
        "Turbo.Tracks.Contracts",
        "Turbo.Tracks.Infrastructure",
        "Turbo.Tracks.Api",
    ];

    private static readonly string[] GeoAssemblies =
    [
        "Turbo.Geo.Core",
        "Turbo.Geo.Contracts",
        "Turbo.Geo.Infrastructure",
        "Turbo.Geo.Api",
    ];

    private static readonly string[] AuthAssemblies =
    [
        "Turbo.Auth.Core",
        "Turbo.Auth.Contracts",
        "Turbo.Auth.Infrastructure",
        "Turbo.Auth.Api",
    ];

    private static readonly string[] CollectionsAssemblies =
    [
        "Turbo.Collections.Core",
        "Turbo.Collections.Contracts",
        "Turbo.Collections.Infrastructure",
        "Turbo.Collections.Api",
    ];

    // Activities is decomposed into a shared parent plus a sub-module per
    // kind (Fishing today; Backcountry Ski etc. later). Each kind's four
    // assemblies sit alongside the shared four; cross-module rules below
    // treat the whole list as one bounded context for the per-module-vs-
    // -others assertions, while ActivityKindBoundaries (below) asserts that
    // no kind references another kind directly.
    private static readonly string[] ActivitiesAssemblies =
    [
        "Turbo.Activities.Shared.Core",
        "Turbo.Activities.Shared.Contracts",
        "Turbo.Activities.Shared.Infrastructure",
        "Turbo.Activities.Shared.Api",
        "Turbo.Activities.Fishing.Core",
        "Turbo.Activities.Fishing.Contracts",
        "Turbo.Activities.Fishing.Infrastructure",
        "Turbo.Activities.Fishing.Api",
        "Turbo.Activities.BackcountrySki.Core",
        "Turbo.Activities.BackcountrySki.Contracts",
        "Turbo.Activities.BackcountrySki.Infrastructure",
        "Turbo.Activities.BackcountrySki.Api",
        "Turbo.Activities.Hiking.Core",
        "Turbo.Activities.Hiking.Contracts",
        "Turbo.Activities.Hiking.Infrastructure",
        "Turbo.Activities.Hiking.Api",
        "Turbo.Activities.XcSki.Core",
        "Turbo.Activities.XcSki.Contracts",
        "Turbo.Activities.XcSki.Infrastructure",
        "Turbo.Activities.XcSki.Api",
        "Turbo.Activities.Packrafting.Core",
        "Turbo.Activities.Packrafting.Contracts",
        "Turbo.Activities.Packrafting.Infrastructure",
        "Turbo.Activities.Packrafting.Api",
        "Turbo.Activities.Freediving.Core",
        "Turbo.Activities.Freediving.Contracts",
        "Turbo.Activities.Freediving.Infrastructure",
        "Turbo.Activities.Freediving.Api",
    ];

    private static IEnumerable<Assembly> Tracks
    {
        get
        {
            _ = typeof(Turboapi.Tracks.TracksScope);
            return TracksAssemblies.Select(LoadByName);
        }
    }
    private static IEnumerable<Assembly> Geo
    {
        get
        {
            _ = typeof(Turboapi.Geo.GeoScope);
            return GeoAssemblies.Select(LoadByName);
        }
    }
    private static IEnumerable<Assembly> Auth
    {
        get
        {
            _ = typeof(Turboapi.Auth.AuthScope);
            return AuthAssemblies.Select(LoadByName);
        }
    }
    private static IEnumerable<Assembly> Collections
    {
        get
        {
            _ = typeof(Turboapi.Collections.CollectionsScope);
            return CollectionsAssemblies.Select(LoadByName);
        }
    }
    private static IEnumerable<Assembly> Activities
    {
        get
        {
            _ = typeof(Turboapi.Activities.ActivitiesScope);
            _ = typeof(Turboapi.Activities.Fishing.FishingScope);
            _ = typeof(Turboapi.Activities.BackcountrySki.BackcountrySkiScope);
            _ = typeof(Turboapi.Activities.Hiking.HikingScope);
            _ = typeof(Turboapi.Activities.XcSki.XcSkiScope);
            _ = typeof(Turboapi.Activities.Packrafting.PackraftingScope);
            _ = typeof(Turboapi.Activities.Freediving.FreedivingScope);
            return ActivitiesAssemblies.Select(LoadByName);
        }
    }

    [Fact]
    public void Tracks_does_not_reference_other_module_assemblies()
    {
        foreach (var module in Tracks)
            AssertNoCrossModuleReference(module,
                forbidden: GeoAssemblies.Concat(AuthAssemblies).Concat(CollectionsAssemblies).Concat(ActivitiesAssemblies).ToArray());
    }

    [Fact]
    public void Geo_does_not_reference_other_module_assemblies()
    {
        foreach (var module in Geo)
            AssertNoCrossModuleReference(module,
                forbidden: TracksAssemblies.Concat(AuthAssemblies).Concat(CollectionsAssemblies).Concat(ActivitiesAssemblies).ToArray());
    }

    [Fact]
    public void Auth_does_not_reference_other_module_assemblies()
    {
        foreach (var module in Auth)
            AssertNoCrossModuleReference(module,
                forbidden: TracksAssemblies.Concat(GeoAssemblies).Concat(CollectionsAssemblies).Concat(ActivitiesAssemblies).ToArray());
    }

    [Fact]
    public void Collections_does_not_reference_other_module_assemblies()
    {
        foreach (var module in Collections)
            AssertNoCrossModuleReference(module,
                forbidden: TracksAssemblies.Concat(GeoAssemblies).Concat(AuthAssemblies).Concat(ActivitiesAssemblies).ToArray());
    }

    [Fact]
    public void Activities_do_not_reference_other_module_assemblies()
    {
        foreach (var module in Activities)
            AssertNoCrossModuleReference(module,
                forbidden: TracksAssemblies.Concat(GeoAssemblies).Concat(AuthAssemblies).Concat(CollectionsAssemblies).ToArray());
    }

    private static Assembly LoadByName(string assemblyName)
    {
        var loaded = AppDomain.CurrentDomain.GetAssemblies()
            .FirstOrDefault(a => a.GetName().Name == assemblyName);
        if (loaded is not null) return loaded;
        return Assembly.Load(assemblyName);
    }

    private static void AssertNoCrossModuleReference(Assembly module, string[] forbidden)
    {
        var referenced = module.GetReferencedAssemblies()
            .Select(a => a.Name)
            .Where(n => n is not null)
            .ToHashSet()!;

        var violations = forbidden.Where(referenced.Contains!).ToList();

        violations.Should().BeEmpty(
            $"{module.GetName().Name} must communicate with other modules only through events on the shared messaging abstractions; "
            + $"direct assembly reference defeats the modulith/microservice symmetry. Forbidden references found: {string.Join(", ", violations)}");
    }
}
