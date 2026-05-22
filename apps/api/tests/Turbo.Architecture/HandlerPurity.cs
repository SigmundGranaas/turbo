using System.Reflection;
using FluentAssertions;
using NetArchTest.Rules;
using Xunit;

namespace Turbo.Architecture;

/// <summary>
/// Encodes the user's "the wiring is deployment and app infra concern, not
/// a core app concern" requirement at the handler level. Command and query
/// handlers must reach storage only through the per-module
/// <see cref="Turbo.Outbox.IUnitOfWork{TScope}"/> and
/// <see cref="Turbo.Outbox.IOutbox{TScope}"/> abstractions — never via EF
/// Core's DbContext, never via Npgsql. The marker types (TracksScope,
/// GeoScope, AuthScope) keep handlers free of any EF type names.
/// </summary>
public sealed class HandlerPurity
{
    private static Assembly LoadByName(string assemblyName)
    {
        var loaded = AppDomain.CurrentDomain.GetAssemblies()
            .FirstOrDefault(a => a.GetName().Name == assemblyName);
        if (loaded is not null) return loaded;
        return Assembly.Load(assemblyName);
    }

    private static readonly string[] ForbiddenInHandlers =
    {
        "Microsoft.EntityFrameworkCore",
        "Npgsql",
        "NATS.Client",
        "NATS.Net",
        "Confluent.Kafka",
    };

    [Fact]
    public void Tracks_command_handlers_do_not_reference_EF_Core_or_Npgsql()
    {
        _ = typeof(Turboapi.Tracks.domain.handler.CreateTrackHandler);
        var assembly = LoadByName("Turbo.Tracks.Core");

        var result = Types.InAssembly(assembly)
            .That()
            .ResideInNamespace("Turboapi.Tracks.domain.handler")
            .ShouldNot()
            .HaveDependencyOnAny(ForbiddenInHandlers)
            .GetResult();

        result.IsSuccessful.Should().BeTrue(
            $"Tracks command handlers must talk to storage only through IUnitOfWork<TracksScope> / IOutbox<TracksScope>; offending: {Describe(result.FailingTypes)}");
    }

    [Fact]
    public void Geo_command_handlers_do_not_reference_EF_Core_or_Npgsql()
    {
        _ = typeof(Turboapi.Geo.domain.handler.CreateLocationHandler);
        var assembly = LoadByName("Turbo.Geo.Core");

        var result = Types.InAssembly(assembly)
            .That()
            .ResideInNamespace("Turboapi.Geo.domain.handler")
            .ShouldNot()
            .HaveDependencyOnAny(ForbiddenInHandlers)
            .GetResult();

        result.IsSuccessful.Should().BeTrue(
            $"Geo command handlers must talk to storage only through IUnitOfWork<GeoScope> / IOutbox<GeoScope>; offending: {Describe(result.FailingTypes)}");
    }

    [Fact]
    public void Collections_command_handlers_do_not_reference_EF_Core_or_Npgsql()
    {
        _ = typeof(Turboapi.Collections.domain.handler.CreateCollectionHandler);
        var assembly = LoadByName("Turbo.Collections.Core");

        var result = Types.InAssembly(assembly)
            .That()
            .ResideInNamespace("Turboapi.Collections.domain.handler")
            .ShouldNot()
            .HaveDependencyOnAny(ForbiddenInHandlers)
            .GetResult();

        result.IsSuccessful.Should().BeTrue(
            $"Collections command handlers must talk to storage only through IUnitOfWork<CollectionsScope> / IOutbox<CollectionsScope>; offending: {Describe(result.FailingTypes)}");
    }

    [Fact]
    public void Auth_use_case_handlers_do_not_reference_EF_Core_or_Npgsql()
    {
        _ = typeof(Turboapi.Auth.Application.UseCases.Commands.RegisterUserWithPassword.RegisterUserWithPasswordCommandHandler);
        var assembly = LoadByName("Turbo.Auth.Core");

        var result = Types.InAssembly(assembly)
            .That()
            .ResideInNamespaceMatching(@"Turboapi\.Auth\.Application\.UseCases\..*")
            .ShouldNot()
            .HaveDependencyOnAny(ForbiddenInHandlers)
            .GetResult();

        result.IsSuccessful.Should().BeTrue(
            $"Auth command/query handlers must talk to storage only through IUnitOfWork / IOutbox<AuthScope>; offending: {Describe(result.FailingTypes)}");
    }

    private static string Describe(IEnumerable<Type>? failing)
        => failing is null ? "<none>" : string.Join(", ", failing.Select(t => t.FullName));
}
