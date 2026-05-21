using DotNet.Testcontainers.Builders;
using DotNet.Testcontainers.Containers;
using Testcontainers.PostgreSql;

namespace Turbo.Behaviour.Testing;

/// <summary>
/// Factory for the Postgres + NATS containers every behaviour fixture
/// needs. Centralizes the image tags and the port-binding / healthcheck
/// flags so changing the Postgres image (eg upgrading PostGIS) happens
/// in one place.
/// </summary>
public static class TurboTestContainers
{
    /// <summary>
    /// Postgres with PostGIS — Geo and the modulith both need it; the
    /// other modules don't care but PostGIS-on-top-of-Postgres is a
    /// superset, so we use it everywhere for consistency.
    /// </summary>
    public static PostgreSqlContainer PostgresWithPostGis(string database = "postgres")
        => new PostgreSqlBuilder()
            .WithImage("postgis/postgis:17-3.5-alpine")
            .WithDatabase(database)
            .WithUsername("postgres")
            .WithPassword("postgres")
            .Build();

    /// <summary>
    /// NATS with JetStream. <c>-js</c> turns the stream subsystem on; we
    /// bind a random host port so multiple fixtures can run in parallel
    /// without colliding.
    /// </summary>
    public static IContainer NatsJetStream()
        => new ContainerBuilder()
            .WithImage("nats:2.10-alpine")
            .WithCommand("-js")
            .WithPortBinding(4222, true)
            .WithWaitStrategy(Wait.ForUnixContainer().UntilPortIsAvailable(4222))
            .Build();

    public static string NatsUrl(IContainer container)
        => $"nats://{container.Hostname}:{container.GetMappedPublicPort(4222)}";
}
