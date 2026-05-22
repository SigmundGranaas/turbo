using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.BackcountrySki.events;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.value;

namespace Turboapi.Activities.BackcountrySki.domain.handler;

public sealed class UpdateBackcountrySkiActivityHandler
{
    private readonly IBackcountrySkiActivityReader _reader;
    private readonly IOwnerGuard _ownerGuard;
    private readonly IGeometryNormalizer _geom;
    private readonly IOutbox<BackcountrySkiScope> _outbox;
    private readonly IUnitOfWork<BackcountrySkiScope> _uow;

    public UpdateBackcountrySkiActivityHandler(
        IBackcountrySkiActivityReader reader,
        IOwnerGuard ownerGuard,
        IGeometryNormalizer geom,
        IOutbox<BackcountrySkiScope> outbox,
        IUnitOfWork<BackcountrySkiScope> uow)
    {
        _reader = reader;
        _ownerGuard = ownerGuard;
        _geom = geom;
        _outbox = outbox;
        _uow = uow;
    }

    public async Task Handle(UpdateBackcountrySkiActivityCommand cmd)
    {
        var existing = await _reader.GetByIdAsync(cmd.ActivityId)
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

        var updated = new BackcountrySkiActivityUpdated(
            next.Core.Id, next.Core.OwnerId,
            cmd.Name, cmd.Description, cmd.RouteWkt,
            cmd.Details, next.Core.Version);

        var summary = new ActivitySummaryUpserted(
            activityId: next.Core.Id,
            ownerId: next.Core.OwnerId,
            kind: "backcountry_ski",
            name: next.Core.Name,
            geometry: new ActivityGeometryWkt(ActivityGeometryKind.LineString, new WKTWriter().Write(next.Core.Geometry)),
            iconKey: "backcountry_ski",
            colorHex: "#7A3CCB",
            version: next.Core.Version);

        var events = new DomainEvent[] { updated, summary };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(next.Core.Id, events, ct));
    }
}
