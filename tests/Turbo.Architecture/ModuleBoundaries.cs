using System.Reflection;
using FluentAssertions;
using Xunit;

namespace Turbo.Architecture;

/// <summary>
/// The three modules must not have direct references to one another. If
/// Activity ever depended on Geo (or vice versa) the "messages only"
/// contract would silently degrade to "messages plus a sneaky compile-time
/// call." The shared abstraction packages (Turbo.Messaging.*,
/// Turbo.Outbox.*, Turbo.Messaging.Nats / InProcess) are the only legal way
/// for one module to reach another.
/// </summary>
public sealed class ModuleBoundaries
{
    private static readonly string[] ActivityAssemblies =
    [
        "Turbo.Activity.Core",
        "Turbo.Activity.Contracts",
        "Turbo.Activity.Infrastructure",
        "Turbo.Activity.Api",
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

    private static IEnumerable<Assembly> Activity =>
        ActivityAssemblies.Select(LoadByName);
    private static IEnumerable<Assembly> Geo
    {
        get
        {
            _ = typeof(Turboapi.Geo.GeoScope); // force-load Contracts
            return GeoAssemblies.Select(LoadByName);
        }
    }
    private static IEnumerable<Assembly> Auth
    {
        get
        {
            _ = typeof(Turboapi.Auth.AuthScope); // force-load Contracts
            return AuthAssemblies.Select(LoadByName);
        }
    }

    [Fact]
    public void Activity_does_not_reference_Geo_or_Auth_assemblies()
    {
        _ = typeof(Turboapi.Activity.ActivityScope); // force-load
        foreach (var module in Activity)
            AssertNoCrossModuleReference(module, forbidden: GeoAssemblies.Concat(AuthAssemblies).ToArray());
    }

    [Fact]
    public void Geo_does_not_reference_Activity_or_Auth_assemblies()
    {
        foreach (var module in Geo)
            AssertNoCrossModuleReference(module, forbidden: ActivityAssemblies.Concat(AuthAssemblies).ToArray());
    }

    [Fact]
    public void Auth_does_not_reference_Activity_or_Geo_assemblies()
    {
        foreach (var module in Auth)
            AssertNoCrossModuleReference(module, forbidden: ActivityAssemblies.Concat(GeoAssemblies).ToArray());
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
