using Microsoft.AspNetCore.Hosting;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Turbo.Behaviour.Testing;
using Turbo.Hosting.Postgres;
using Turboapi.Collections.data;
using Turboapi.Sharing;

namespace Turbo.Collections.Behaviour;

public sealed class CollectionsHostFixture : TurboHostFixture<Turbo.Host.Collections.CollectionsHostProgram>
{
    public CollectionsHostFixture() : base("collections") { }

    protected override string ConnectionStringKey => "Collections";

    protected override IDictionary<string, string?> ExtraSettings => new Dictionary<string, string?>
    {
        // The standalone Collections host requires ConnectionStrings:Sharing
        // to wire its IAccessControl. The fixture replaces IAccessControl
        // below with one backed by the collections read-model OwnerId
        // (preserving the legacy owner-only contract this suite asserts),
        // so the actual sharing schema is never consulted; the value here
        // is a placeholder the real DbContext never opens.
        ["ConnectionStrings:Sharing"] = ConnectionString,
    };

    protected override void ConfigureTestServices(WebHostBuilderContext context, IServiceCollection services)
    {
        ReplaceDbContext<CollectionsReadContext>(services,
            o => o.UseNpgsql(ConnectionString));
        // OwnerId-backed IAccessControl: this suite asserts the pre-sharing
        // contract that strangers cannot read other users' collections.
        // Cross-user / grant-driven access is covered end-to-end by the
        // microservices-topology tests which run the real EfAccessControl
        // against a real Sharing schema with real grants.
        services.RemoveAll<IAccessControl>();
        services.AddScoped<IAccessControl, OwnerIdBackedAccessControl>();
    }

    protected override Task MigrateAsync(IServiceProvider services)
        => services.MigrateModuleDatabaseAsync<CollectionsReadContext>(ConnectionString);
}

/// <summary>
/// Test-only IAccessControl that mirrors the pre-sharing OwnerId check:
/// returns Owner if the calling user matches the collection's OwnerId,
/// null otherwise. Used by the Collections behaviour suite to preserve
/// the "stranger cannot read someone else's collection" property that
/// pre-dates the unified sharing primitive.
/// </summary>
internal sealed class OwnerIdBackedAccessControl : IAccessControl
{
    private readonly CollectionsReadContext _db;
    public OwnerIdBackedAccessControl(CollectionsReadContext db) => _db = db;

    public async Task<bool> CanReadAsync(Guid userId, Guid resourceId, CancellationToken ct = default)
        => await EffectiveRoleAsync(userId, resourceId, ct) is not null;

    public async Task<bool> CanWriteAsync(Guid userId, Guid resourceId, CancellationToken ct = default)
        => await EffectiveRoleAsync(userId, resourceId, ct) is EffectiveRole.Owner;

    public async Task<EffectiveRole?> EffectiveRoleAsync(Guid userId, Guid resourceId, CancellationToken ct = default)
    {
        var owner = await _db.Collections
            .AsNoTracking()
            .Where(c => c.Id == resourceId && c.DeletedAt == null)
            .Select(c => (Guid?)c.OwnerId)
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
