using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.Fishing.events;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Fishing.domain.handler;

public sealed class UpdateFishingActivityHandler
{
    private readonly IFishingActivityReader _reader;
    private readonly IOwnerGuard _ownerGuard;
    private readonly IGeometryNormalizer _geom;
    private readonly IOutbox<FishingScope> _outbox;
    private readonly IUnitOfWork<FishingScope> _uow;

    public UpdateFishingActivityHandler(
        IFishingActivityReader reader,
        IOwnerGuard ownerGuard,
        IGeometryNormalizer geom,
        IOutbox<FishingScope> outbox,
        IUnitOfWork<FishingScope> uow)
    {
        _reader = reader;
        _ownerGuard = ownerGuard;
        _geom = geom;
        _outbox = outbox;
        _uow = uow;
    }

    public async Task Handle(UpdateFishingActivityCommand cmd)
    {
        // The fishing projection is async; a fast client that POSTs and
        // then PATCHes the new id can race the projector. Brief retry
        // before declaring the activity missing — see
        // <see cref="ReadModelCatchup"/>.
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
        if (cmd.Longitude is not null && cmd.Latitude is not null)
        {
            var factory = new GeometryFactory(new PrecisionModel(), 4326);
            var p = factory.CreatePoint(new Coordinate(cmd.Longitude.Value, cmd.Latitude.Value));
            next = next.Relocate((Point)_geom.Normalize(p, ActivityGeometryKind.Point));
            changed = true;
        }
        if (cmd.Details is not null)
        {
            next = next.ReplaceDetails(cmd.Details);
            changed = true;
        }
        if (!changed) return;

        var updated = new FishingActivityUpdated(
            next.Core.Id, next.Core.OwnerId,
            cmd.Name, cmd.Description,
            cmd.Longitude, cmd.Latitude,
            cmd.Details, next.Core.Version);

        var summary = new ActivitySummaryUpserted(
            activityId: next.Core.Id,
            ownerId: next.Core.OwnerId,
            kind: "fishing",
            name: next.Core.Name,
            geometry: new ActivityGeometryWkt(ActivityGeometryKind.Point, new WKTWriter().Write(next.Core.Geometry)),
            iconKey: "fishing",
            colorHex: "#1E6FB8",
            version: next.Core.Version);

        var events = new DomainEvent[] { updated, summary };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(next.Core.Id, events, ct));
    }
}
