using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.Hiking.events;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Hiking.domain.handler;

public sealed class UpdateHikingActivityHandler
{
    private readonly IHikingActivityReader _reader;
    private readonly IOwnerGuard _ownerGuard;
    private readonly IGeometryNormalizer _geom;
    private readonly IOutbox<HikingScope> _outbox;
    private readonly IUnitOfWork<HikingScope> _uow;

    public UpdateHikingActivityHandler(
        IHikingActivityReader reader, IOwnerGuard ownerGuard, IGeometryNormalizer geom,
        IOutbox<HikingScope> outbox, IUnitOfWork<HikingScope> uow)
    {
        _reader = reader; _ownerGuard = ownerGuard; _geom = geom; _outbox = outbox; _uow = uow;
    }

    public async Task Handle(UpdateHikingActivityCommand cmd)
    {
        var existing = await ReadModelCatchup.ReadAsync(
                ct => _reader.GetByIdAsync(cmd.ActivityId, ct))
            ?? throw new ActivityNotFoundException(cmd.ActivityId);

        _ownerGuard.RequireOwner(cmd.CallerId, existing.Core.OwnerId);

        if (cmd.IfMatchVersion is { } expected && existing.Core.Version != expected)
            throw new OptimisticConcurrencyException(expected, existing.Core.Version);

        var next = existing;
        var changed = false;
        if (cmd.Name is not null || cmd.Description is not null)
        {
            next = next.Rename(cmd.Name, cmd.Description);
            changed = true;
        }
        if (cmd.RouteWkt is not null)
        {
            var reader = new WKTReader();
            var parsed = reader.Read(cmd.RouteWkt);
            if (parsed.SRID != 4326) parsed.SRID = 4326;
            var route = (LineString)_geom.Normalize(parsed, ActivityGeometryKind.LineString);
            next = next.ReplaceRoute(route);
            changed = true;
        }
        if (cmd.Details is not null)
        {
            next = next.ReplaceDetails(cmd.Details);
            changed = true;
        }
        if (!changed) return;

        var updated = new HikingActivityUpdated(
            next.Core.Id, next.Core.OwnerId,
            cmd.Name, cmd.Description, cmd.RouteWkt,
            cmd.Details, next.Core.Version);

        var summary = new ActivitySummaryUpserted(
            activityId: next.Core.Id, ownerId: next.Core.OwnerId, kind: "hiking",
            name: next.Core.Name,
            geometry: new ActivityGeometryWkt(ActivityGeometryKind.LineString, new WKTWriter().Write(next.Core.Geometry)),
            iconKey: "hiking", colorHex: "#2E7D32", version: next.Core.Version);

        var events = new DomainEvent[] { updated, summary };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(next.Core.Id, events, ct));
    }
}
