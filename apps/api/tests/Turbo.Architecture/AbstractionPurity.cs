using System.Reflection;
using FluentAssertions;
using NetArchTest.Rules;
using Xunit;

namespace Turbo.Architecture;

/// <summary>
/// The user's explicit ask: "the details of the adapters are not important
/// and should never influence the core layers; the wiring is a deployment
/// and app infra concern, not a core app concern." These tests enforce
/// that promise at the package boundary so adapter dependencies cannot
/// creep into the abstractions over time.
/// </summary>
public sealed class AbstractionPurity
{
    private static readonly Assembly MessagingAbstractions =
        typeof(Turbo.Messaging.IDomainEvent).Assembly;

    private static readonly Assembly OutboxAbstractions =
        typeof(Turbo.Outbox.IOutbox<>).Assembly;

    private static readonly Assembly InProcessAdapter =
        typeof(Turbo.Messaging.InProcess.InProcessMessageTransport).Assembly;

    private static readonly Assembly NatsAdapter =
        typeof(Turbo.Messaging.Nats.NatsMessageTransport).Assembly;

    private static readonly string[] ForbiddenForAbstractions =
    {
        "Microsoft.EntityFrameworkCore",
        "Npgsql",
        "NATS.Client",
        "NATS.Net",
        "Confluent.Kafka",
        "Microsoft.AspNetCore",
        "System.Net.Http",
    };

    [Fact]
    public void Turbo_Messaging_Abstractions_has_no_infrastructure_dependencies()
    {
        var result = Types.InAssembly(MessagingAbstractions)
            .ShouldNot()
            .HaveDependencyOnAny(ForbiddenForAbstractions)
            .GetResult();

        result.IsSuccessful.Should().BeTrue(
            $"abstraction types must stay broker- and storage-agnostic; offending types: {Describe(result.FailingTypes)}");
    }

    [Fact]
    public void Turbo_Outbox_Abstractions_has_no_infrastructure_dependencies()
    {
        var result = Types.InAssembly(OutboxAbstractions)
            .ShouldNot()
            .HaveDependencyOnAny(ForbiddenForAbstractions)
            .GetResult();

        result.IsSuccessful.Should().BeTrue(
            $"outbox abstractions must stay storage-agnostic; offending types: {Describe(result.FailingTypes)}");
    }

    [Fact]
    public void Turbo_Messaging_InProcess_does_not_depend_on_NATS_or_Kafka()
    {
        var result = Types.InAssembly(InProcessAdapter)
            .ShouldNot()
            .HaveDependencyOnAny("NATS.Client", "NATS.Net", "Confluent.Kafka")
            .GetResult();

        result.IsSuccessful.Should().BeTrue(
            $"the in-process transport must not pull in any broker SDK; offending types: {Describe(result.FailingTypes)}");
    }

    [Fact]
    public void Turbo_Messaging_Nats_does_not_depend_on_Turbo_Messaging_InProcess()
    {
        var result = Types.InAssembly(NatsAdapter)
            .ShouldNot()
            .HaveDependencyOn("Turbo.Messaging.InProcess")
            .GetResult();

        result.IsSuccessful.Should().BeTrue(
            $"the NATS adapter must be independent of the in-process transport; offending types: {Describe(result.FailingTypes)}");
    }

    private static string Describe(IEnumerable<Type>? failing)
        => failing is null ? "<none>" : string.Join(", ", failing.Select(t => t.FullName));
}
