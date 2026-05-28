using Microsoft.EntityFrameworkCore;
using Turboapi.Sharing.data.model;
using Turboapi.Sharing.domain.service;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.data;

public sealed class EfResourceSyncService : IResourceSyncService
{
    public const int MaxLimit = 500;

    private readonly SharingReadContext _db;

    public EfResourceSyncService(SharingReadContext db) => _db = db;

    public async Task<ResourceSyncPage> SyncAsync(
        Guid userId,
        DateTime? since,
        IReadOnlyCollection<string>? types,
        int limit,
        CancellationToken cancellationToken = default)
    {
        var effectiveLimit = Math.Clamp(limit, 1, MaxLimit);
        var effectiveSince = since ?? DateTime.MinValue.ToUniversalTime();
        var subjectUser = SubjectType.User.ToWire();
        var subjectGroup = SubjectType.Group.ToWire();
        var now = DateTime.UtcNow;

        // Resource ids reachable via grants (direct or group). Resolved in a
        // single round trip; the envelope projection joins on these.
        var grantedIds = _db.Grants
            .AsNoTracking()
            .Where(g =>
                (g.ExpiresAt == null || g.ExpiresAt > now)
                && ((g.SubjectType == subjectUser && g.SubjectId == userId)
                    || (g.SubjectType == subjectGroup
                        && _db.GroupMembers.Any(m => m.GroupId == g.SubjectId && m.UserId == userId))))
            .Select(g => g.ResourceId);

        var query = _db.Resources
            .AsNoTracking()
            .Where(r =>
                r.UpdatedAt > effectiveSince
                && (r.OwnerId == userId || grantedIds.Contains(r.Id)));

        if (types is { Count: > 0 })
        {
            var typeList = types.ToList();
            query = query.Where(r => typeList.Contains(r.Type));
        }

        var rows = await query
            .OrderBy(r => r.UpdatedAt)
            .Take(effectiveLimit)
            .ToListAsync(cancellationToken);

        // Materialize my_role per row. Owner short-circuits; otherwise look
        // up the strongest applicable grant.
        var envelopes = new List<ResourceEnvelopeDto>(rows.Count);
        foreach (var r in rows)
        {
            var role = await ResolveRoleAsync(userId, r, subjectUser, subjectGroup, now, cancellationToken);
            envelopes.Add(new ResourceEnvelopeDto(
                r.Id, r.Type, r.OwnerId, r.Visibility, role,
                r.Version, r.UpdatedAt, r.DeletedAt is not null));
        }

        return new ResourceSyncPage(envelopes, DateTime.UtcNow);
    }

    private async Task<string> ResolveRoleAsync(
        Guid userId, ResourceEntity r,
        string subjectUser, string subjectGroup,
        DateTime now,
        CancellationToken cancellationToken)
    {
        if (r.OwnerId == userId) return "owner";

        var direct = await _db.Grants.AsNoTracking()
            .Where(g => g.ResourceId == r.Id
                        && g.SubjectType == subjectUser
                        && g.SubjectId == userId
                        && (g.ExpiresAt == null || g.ExpiresAt > now))
            .Select(g => g.Role)
            .FirstOrDefaultAsync(cancellationToken);

        var viaGroup = await (from g in _db.Grants.AsNoTracking()
                              join gm in _db.GroupMembers.AsNoTracking()
                                  on g.SubjectId equals gm.GroupId
                              where g.ResourceId == r.Id
                                    && g.SubjectType == subjectGroup
                                    && gm.UserId == userId
                                    && (g.ExpiresAt == null || g.ExpiresAt > now)
                              select g.Role)
                              .FirstOrDefaultAsync(cancellationToken);

        return Promote(direct, viaGroup, r.Visibility == Visibility.Public.ToWire());
    }

    private static string Promote(string? direct, string? group, bool publicViewer)
    {
        var best = "viewer";
        var has = false;
        if (publicViewer) { has = true; }
        if (direct is not null) { has = true; if (direct == "editor") best = "editor"; }
        if (group is not null) { has = true; if (group == "editor") best = "editor"; }
        // No matching subject (shouldn't happen — grantedIds already filtered)
        return has ? best : "viewer";
    }
}
