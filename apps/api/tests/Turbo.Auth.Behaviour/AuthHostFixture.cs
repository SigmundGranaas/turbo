using Microsoft.AspNetCore.Hosting;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Turbo.Behaviour.Testing;
using Turbo.Hosting.Postgres;
using Turboapi.Auth.Infrastructure.Persistence;

namespace Turbo.Auth.Behaviour;

public sealed class AuthHostFixture : TurboHostFixture<Turbo.Host.Auth.AuthHostProgram>
{
    public AuthHostFixture() : base("auth") { }

    protected override string ConnectionStringKey => "Auth";

    protected override void ConfigureTestServices(WebHostBuilderContext context, IServiceCollection services)
        => ReplaceDbContext<AuthDbContext>(services, o => o.UseNpgsql(ConnectionString));

    protected override Task MigrateAsync(IServiceProvider services)
        => services.MigrateModuleDatabaseAsync<AuthDbContext>(ConnectionString);
}
