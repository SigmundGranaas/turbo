using Microsoft.AspNetCore.Hosting;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Turbo.Behaviour.Testing;
using Turbo.Hosting.Postgres;
using Turboapi.Sharing.data;

namespace Turbo.Sharing.Behaviour;

/// <summary>
/// Boots the standalone Turbo.Host.Sharing process against a fresh
/// Postgres + NATS pair. Used for tests that verify the dedicated
/// per-service deploy works end-to-end (HTTP, migrations, DI wiring).
/// Cross-stream NATS subscribers are registered but the peer streams
/// won't exist in this single-host shape — the subscriber host logs
/// and continues, so the test fixture stays usable.
/// </summary>
public sealed class SharingHostFixture : TurboHostFixture<Turbo.Host.Sharing.SharingHostProgram>
{
    public SharingHostFixture() : base("sharing") { }

    protected override string ConnectionStringKey => "Sharing";

    protected override void ConfigureTestServices(WebHostBuilderContext context, IServiceCollection services)
        => ReplaceDbContext<SharingReadContext>(services,
            o => o.UseNpgsql(ConnectionString));

    protected override Task MigrateAsync(IServiceProvider services)
        => services.MigrateModuleDatabaseAsync<SharingReadContext>(ConnectionString);
}
