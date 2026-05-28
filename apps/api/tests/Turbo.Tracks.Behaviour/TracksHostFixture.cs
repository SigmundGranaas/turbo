using Microsoft.AspNetCore.Hosting;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Turbo.Behaviour.Testing;
using Turbo.Hosting.Postgres;
using Turboapi.Sharing;
using Turboapi.Tracks.data;

namespace Turbo.Tracks.Behaviour;

public sealed class TracksHostFixture : TurboHostFixture<Turbo.Host.Tracks.TracksHostProgram>
{
    public TracksHostFixture() : base("tracks") { }

    protected override string ConnectionStringKey => "Tracks";

    protected override IDictionary<string, string?> ExtraSettings => new Dictionary<string, string?>
    {
        // Standalone Tracks host requires ConnectionStrings:Sharing for its
        // IAccessControl. The fixture replaces that with an OwnerId-backed
        // stub below so this suite stays self-contained.
        ["ConnectionStrings:Sharing"] = ConnectionString,
    };

    protected override void ConfigureTestServices(WebHostBuilderContext context, IServiceCollection services)
    {
        ReplaceDbContext<TrackReadContext>(services,
            o => o.UseNpgsql(ConnectionString, x => x.UseNetTopologySuite()));
        services.RemoveAll<IAccessControl>();
        services.AddScoped<IAccessControl, OwnerIdBackedTracksAccessControl>();
    }

    protected override Task MigrateAsync(IServiceProvider services)
        => services.MigrateModuleDatabaseAsync<TrackReadContext>(ConnectionString);
}

/// <summary>
/// OwnerId-backed IAccessControl for the Tracks behaviour suite. Same
/// rationale as the Geo and Collections variants — keeps single-host
/// suites self-contained; cross-user grant flows live in the
/// microservices topology tests.
/// </summary>
internal sealed class OwnerIdBackedTracksAccessControl : IAccessControl
{
    private readonly TrackReadContext _db;
    public OwnerIdBackedTracksAccessControl(TrackReadContext db) => _db = db;

    public async Task<bool> CanReadAsync(Guid userId, Guid resourceId, CancellationToken ct = default)
        => await EffectiveRoleAsync(userId, resourceId, ct) is not null;

    public async Task<bool> CanWriteAsync(Guid userId, Guid resourceId, CancellationToken ct = default)
        => await EffectiveRoleAsync(userId, resourceId, ct) is EffectiveRole.Owner;

    public async Task<EffectiveRole?> EffectiveRoleAsync(Guid userId, Guid resourceId, CancellationToken ct = default)
    {
        var owner = await _db.Tracks
            .AsNoTracking()
            .Where(t => t.Id == resourceId && t.DeletedAt == null)
            .Select(t => (Guid?)t.OwnerId)
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
