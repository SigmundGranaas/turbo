using Microsoft.AspNetCore.Hosting;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Turbo.Behaviour.Testing;
using Turbo.Hosting.Postgres;
using Turboapi.Collections.data;

namespace Turbo.Collections.Behaviour;

public sealed class CollectionsHostFixture : TurboHostFixture<Turbo.Host.Collections.CollectionsHostProgram>
{
    public CollectionsHostFixture() : base("collections") { }

    protected override string ConnectionStringKey => "Collections";

    protected override void ConfigureTestServices(WebHostBuilderContext context, IServiceCollection services)
        => ReplaceDbContext<CollectionsReadContext>(services,
            o => o.UseNpgsql(ConnectionString));

    protected override Task MigrateAsync(IServiceProvider services)
        => services.MigrateModuleDatabaseAsync<CollectionsReadContext>(ConnectionString);
}
