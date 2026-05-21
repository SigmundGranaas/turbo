using System.Reflection;
using FluentAssertions;
using Xunit;

namespace Turbo.Architecture;

/// <summary>
/// Each activity kind is its own sub-module (its own four assemblies)
/// and must not directly reference any other kind. The shared activities
/// assemblies are the only legal way for one kind to interact with
/// another — and even that is mediated by events on the bus, never by
/// type calls. Adding a new kind = add to <see cref="KindAssemblyLists"/>;
/// the test fails if the new kind sneaks a `ProjectReference` to a sibling.
/// </summary>
public sealed class ActivityKindBoundaries
{
    private static readonly Dictionary<string, string[]> KindAssemblyLists = new()
    {
        ["Fishing"] = new[]
        {
            "Turbo.Activities.Fishing.Core",
            "Turbo.Activities.Fishing.Contracts",
            "Turbo.Activities.Fishing.Infrastructure",
            "Turbo.Activities.Fishing.Api",
        },
        ["BackcountrySki"] = new[]
        {
            "Turbo.Activities.BackcountrySki.Core",
            "Turbo.Activities.BackcountrySki.Contracts",
            "Turbo.Activities.BackcountrySki.Infrastructure",
            "Turbo.Activities.BackcountrySki.Api",
        },
        ["Hiking"] = new[]
        {
            "Turbo.Activities.Hiking.Core",
            "Turbo.Activities.Hiking.Contracts",
            "Turbo.Activities.Hiking.Infrastructure",
            "Turbo.Activities.Hiking.Api",
        },
        ["XcSki"] = new[]
        {
            "Turbo.Activities.XcSki.Core",
            "Turbo.Activities.XcSki.Contracts",
            "Turbo.Activities.XcSki.Infrastructure",
            "Turbo.Activities.XcSki.Api",
        },
        ["Packrafting"] = new[]
        {
            "Turbo.Activities.Packrafting.Core",
            "Turbo.Activities.Packrafting.Contracts",
            "Turbo.Activities.Packrafting.Infrastructure",
            "Turbo.Activities.Packrafting.Api",
        },
        ["Freediving"] = new[]
        {
            "Turbo.Activities.Freediving.Core",
            "Turbo.Activities.Freediving.Contracts",
            "Turbo.Activities.Freediving.Infrastructure",
            "Turbo.Activities.Freediving.Api",
        },
    };

    [Fact]
    public void No_activity_kind_references_another_activity_kind_directly()
    {
        // Touch each kind's scope so the assemblies are loaded.
        _ = typeof(Turboapi.Activities.Fishing.FishingScope);
        _ = typeof(Turboapi.Activities.BackcountrySki.BackcountrySkiScope);
        _ = typeof(Turboapi.Activities.Hiking.HikingScope);
        _ = typeof(Turboapi.Activities.XcSki.XcSkiScope);
        _ = typeof(Turboapi.Activities.Packrafting.PackraftingScope);
        _ = typeof(Turboapi.Activities.Freediving.FreedivingScope);

        var kinds = KindAssemblyLists.Keys.ToArray();
        foreach (var kind in kinds)
        {
            var ours = KindAssemblyLists[kind];
            var forbidden = kinds.Where(k => k != kind).SelectMany(k => KindAssemblyLists[k]).ToArray();

            foreach (var assemblyName in ours)
            {
                var asm = LoadByName(assemblyName);
                var referenced = asm.GetReferencedAssemblies().Select(a => a.Name).ToHashSet();
                var violations = forbidden.Where(referenced.Contains!).ToList();
                violations.Should().BeEmpty(
                    $"kind {kind}'s assembly {assemblyName} must not reference another kind's assemblies; "
                    + "cross-kind interaction goes through events on the bus, not type references. Forbidden: "
                    + string.Join(", ", violations));
            }
        }
    }

    private static Assembly LoadByName(string assemblyName)
    {
        var loaded = AppDomain.CurrentDomain.GetAssemblies()
            .FirstOrDefault(a => a.GetName().Name == assemblyName);
        if (loaded is not null) return loaded;
        return Assembly.Load(assemblyName);
    }
}
