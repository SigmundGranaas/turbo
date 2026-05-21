using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.domain;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.Fishing.events;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Fishing.domain.handler;

/// <summary>
/// Creates a fishing activity. Composition root: pulls
/// <see cref="IGeometryNormalizer"/> for the SRID/range guard and
/// <see cref="IOutbox{FishingScope}"/> for the durable event publish.
/// The handler emits two events in the same outbox transaction — a
/// kind-specific <see cref="FishingActivityCreated"/> that the typed
/// fishing read-model projector consumes, and a cross-kind
/// <see cref="ActivitySummaryUpserted"/> that the shared summaries
/// projector consumes.
/// </summary>
public sealed class CreateFishingActivityHandler
{
    private readonly IGeometryNormalizer _geom;
    private readonly IOutbox<FishingScope> _outbox;
    private readonly IUnitOfWork<FishingScope> _uow;

    public CreateFishingActivityHandler(
        IGeometryNormalizer geom,
        IOutbox<FishingScope> outbox,
        IUnitOfWork<FishingScope> uow)
    {
        _geom = geom;
        _outbox = outbox;
        _uow = uow;
    }

    public async Task<Guid> Handle(CreateFishingActivityCommand cmd)
    {
        var factory = new GeometryFactory(new PrecisionModel(), 4326);
        var point = factory.CreatePoint(new Coordinate(cmd.Longitude, cmd.Latitude));
        var normalized = _geom.Normalize(point, ActivityGeometryKind.Point);

        var core = ActivityCore.New(cmd.CallerId, cmd.Name, cmd.Description, normalized);
        var fishing = FishingActivity.Create(core, cmd.Details);

        var created = new FishingActivityCreated(
            core.Id, core.OwnerId, core.Name, core.Description,
            cmd.Longitude, cmd.Latitude, cmd.Details);

        var summary = new ActivitySummaryUpserted(
            activityId: core.Id,
            ownerId: core.OwnerId,
            kind: "fishing",
            name: core.Name,
            geometry: new ActivityGeometryWkt(ActivityGeometryKind.Point, new WKTWriter().Write(normalized)),
            iconKey: "fishing",
            colorHex: "#1E6FB8",
            version: core.Version);

        var events = new DomainEvent[] { created, summary };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(core.Id, events, ct));
        return core.Id;
    }
}
