using Microsoft.AspNetCore.Hosting;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Turbo.Behaviour.Testing;
using Turbo.Hosting.Postgres;
using Turboapi.Geo.data.model;
using Turboapi.Geo.domain.query.model;
using Turboapi.Sharing;

namespace Turbo.Geo.Behaviour;

public sealed class GeoHostFixture : TurboHostFixture<Turbo.Host.Geo.GeoHostProgram>
{
    public GeoHostFixture() : base("geo") { }

    protected override string ConnectionStringKey => "Geo";

    protected override IDictionary<string, string?> ExtraSettings => new Dictionary<string, string?>
    {
        // Standalone Geo host requires ConnectionStrings:Sharing for its
        // IAccessControl. The fixture replaces that with an OwnerId-backed
        // stub below so this suite stays self-contained.
        ["ConnectionStrings:Sharing"] = ConnectionString,
    };

    protected override void ConfigureTestServices(WebHostBuilderContext context, IServiceCollection services)
    {
        ReplaceDbContext<LocationReadContext>(services,
            o => o.UseNpgsql(ConnectionString, x => x.UseNetTopologySuite()));
        services.RemoveAll<IAccessControl>();
        services.AddScoped<IAccessControl, OwnerIdBackedGeoAccessControl>();
    }

    protected override Task MigrateAsync(IServiceProvider services)
        => services.MigrateModuleDatabaseAsync<LocationReadContext>(ConnectionString);
}

/// <summary>
/// OwnerId-backed IAccessControl for the Geo behaviour suite. Mirrors the
/// pre-sharing single-tenant contract — strangers see other users'
/// markers as 404. Cross-user grant flows are covered by the
/// microservices-topology tests.
/// </summary>
internal sealed class OwnerIdBackedGeoAccessControl : IAccessControl
{
    private readonly LocationReadContext _db;
    public OwnerIdBackedGeoAccessControl(LocationReadContext db) => _db = db;

    public async Task<bool> CanReadAsync(Guid userId, Guid resourceId, CancellationToken ct = default)
        => await EffectiveRoleAsync(userId, resourceId, ct) is not null;

    public async Task<bool> CanWriteAsync(Guid userId, Guid resourceId, CancellationToken ct = default)
        => await EffectiveRoleAsync(userId, resourceId, ct) is EffectiveRole.Owner;

    public async Task<EffectiveRole?> EffectiveRoleAsync(Guid userId, Guid resourceId, CancellationToken ct = default)
    {
        var owner = await _db.Locations
            .AsNoTracking()
            .Where(l => l.Id == resourceId && l.DeletedAt == null)
            .Select(l => (Guid?)l.OwnerId)
            .FirstOrDefaultAsync(ct);
        if (owner is null) return null;
        return owner == userId ? EffectiveRole.Owner : null;
    }

    public async Task RequireReadAsync(Guid userId, Guid resourceId, CancellationToken ct = default)
    {
        if (!await CanReadAsync(userId, resourceId, ct))
            throw new AccessDeniedException(userId, resourceId);
    }

    public async Task RequireWriteAsync(Guid userId, Guid resourceId, CancellationToken ct = default)
    {
        if (!await CanWriteAsync(userId, resourceId, ct))
            throw new AccessDeniedException(userId, resourceId);
    }
}
