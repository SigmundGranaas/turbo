using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace Turboapi.Auth.Infrastructure.Persistence;

/// <summary>
/// Lets `dotnet ef migrations add` / `dotnet ef dbcontext info` build an
/// <see cref="AuthDbContext"/> without bootstrapping the host. The
/// connection string is design-time only; nothing in this factory affects
/// production wiring (the host's DI registration owns that).
/// </summary>
public sealed class AuthDbContextDesignFactory : IDesignTimeDbContextFactory<AuthDbContext>
{
    public AuthDbContext CreateDbContext(string[] args)
    {
        var options = new DbContextOptionsBuilder<AuthDbContext>()
            .UseNpgsql("Host=localhost;Port=5432;Database=auth;Username=postgres;Password=postgres")
            .Options;
        return new AuthDbContext(options);
    }
}
