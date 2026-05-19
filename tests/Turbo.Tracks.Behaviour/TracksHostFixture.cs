using Microsoft.AspNetCore.Hosting;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Turbo.Behaviour.Testing;
using Turbo.Hosting.Postgres;
using Turboapi.Tracks.data;

namespace Turbo.Tracks.Behaviour;

public sealed class TracksHostFixture : TurboHostFixture<Turbo.Host.Tracks.TracksHostProgram>
{
    public TracksHostFixture() : base("tracks") { }

    protected override string ConnectionStringKey => "Tracks";

    protected override void ConfigureTestServices(WebHostBuilderContext context, IServiceCollection services)
        => ReplaceDbContext<TrackReadContext>(services,
            o => o.UseNpgsql(ConnectionString, x => x.UseNetTopologySuite()));

    protected override Task MigrateAsync(IServiceProvider services)
        => services.MigrateModuleDatabaseAsync<TrackReadContext>(ConnectionString);
}
