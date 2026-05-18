using System.Reflection;
using FluentAssertions;
using Xunit;

namespace Turbo.Architecture;

/// <summary>
/// Each module is split into four assemblies — Core, Contracts,
/// Infrastructure, Api — and the layering rules between them are part of
/// the architecture contract. Core may not depend on EF Core, Npgsql,
/// NATS, ASP.NET, HttpClient. Contracts may not depend on Core. The Api
/// layer must reference its own Infrastructure (so controllers can find
/// the DbContext through DI registrations) but never another module's
/// internals.
/// </summary>
public sealed class ModuleLayering
{
    private static readonly string[] ForbiddenInCore =
    {
        "Microsoft.EntityFrameworkCore",
        "Microsoft.EntityFrameworkCore.Relational",
        "Npgsql",
        "Npgsql.EntityFrameworkCore.PostgreSQL",
        "NATS.Client",
        "NATS.Net",
        "Microsoft.AspNetCore",
        "Microsoft.AspNetCore.Mvc",
        "Microsoft.AspNetCore.Authentication.JwtBearer",
        "System.IdentityModel.Tokens.Jwt",
        "System.Net.Http",
    };

    private static readonly string[] ForbiddenInContracts =
    {
        "Microsoft.EntityFrameworkCore",
        "Npgsql",
        "NATS.Client",
        "NATS.Net",
        "Microsoft.AspNetCore",
    };

    [Fact]
    public void Activity_Core_does_not_depend_on_infrastructure_packages()
    {
        AssertNoForbiddenReferences("Turbo.Activity.Core", ForbiddenInCore);
    }

    [Fact]
    public void Activity_Contracts_does_not_depend_on_infrastructure_or_Core()
    {
        AssertNoForbiddenReferences("Turbo.Activity.Contracts",
            ForbiddenInContracts.Concat(new[] { "Turbo.Activity.Core" }).ToArray());
    }

    [Fact]
    public void Geo_Core_does_not_depend_on_infrastructure_packages()
    {
        AssertNoForbiddenReferences("Turbo.Geo.Core", ForbiddenInCore);
    }

    [Fact]
    public void Geo_Contracts_does_not_depend_on_infrastructure_or_Core()
    {
        AssertNoForbiddenReferences("Turbo.Geo.Contracts",
            ForbiddenInContracts.Concat(new[] { "Turbo.Geo.Core" }).ToArray());
    }

    [Fact]
    public void Auth_Core_does_not_depend_on_infrastructure_packages()
    {
        AssertNoForbiddenReferences("Turbo.Auth.Core", ForbiddenInCore);
    }

    [Fact]
    public void Auth_Contracts_does_not_depend_on_infrastructure_or_Core()
    {
        AssertNoForbiddenReferences("Turbo.Auth.Contracts",
            ForbiddenInContracts.Concat(new[] { "Turbo.Auth.Core" }).ToArray());
    }

    private static void AssertNoForbiddenReferences(string assemblyName, string[] forbidden)
    {
        var assembly = LoadByName(assemblyName);
        var referenced = assembly.GetReferencedAssemblies()
            .Select(a => a.Name)
            .Where(n => n is not null)
            .ToHashSet()!;

        var violations = forbidden.Where(referenced.Contains!).ToList();

        violations.Should().BeEmpty(
            $"{assemblyName} must stay free of these dependencies. Forbidden references found: {string.Join(", ", violations)}");
    }

    private static Assembly LoadByName(string name)
    {
        var loaded = AppDomain.CurrentDomain.GetAssemblies()
            .FirstOrDefault(a => a.GetName().Name == name);
        return loaded ?? Assembly.Load(name);
    }
}
