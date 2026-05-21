using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.BackcountrySki.events;
using Turboapi.Activities.domain;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.value;

namespace Turboapi.Activities.BackcountrySki.domain.handler;

public sealed class CreateBackcountrySkiActivityHandler
{
    private readonly IGeometryNormalizer _geom;
    private readonly IOutbox<BackcountrySkiScope> _outbox;
    private readonly IUnitOfWork<BackcountrySkiScope> _uow;

    public CreateBackcountrySkiActivityHandler(
        IGeometryNormalizer geom,
        IOutbox<BackcountrySkiScope> outbox,
        IUnitOfWork<BackcountrySkiScope> uow)
    {
        _geom = geom;
        _outbox = outbox;
        _uow = uow;
    }

    public async Task<Guid> Handle(CreateBackcountrySkiActivityCommand cmd)
    {
        var reader = new WKTReader();
        var parsed = reader.Read(cmd.RouteWkt);
        if (parsed.SRID != 4326) parsed.SRID = 4326;
        var route = (LineString)_geom.Normalize(parsed, ActivityGeometryKind.LineString);

        var core = ActivityCore.New(cmd.CallerId, cmd.Name, cmd.Description, route);
        _ = BackcountrySkiActivity.Create(core, cmd.Details); // validates invariants

        var created = new BackcountrySkiActivityCreated(
            core.Id, core.OwnerId, core.Name, core.Description,
            new WKTWriter().Write(route), cmd.Details);

        var summary = new ActivitySummaryUpserted(
            activityId: core.Id,
            ownerId: core.OwnerId,
            kind: "backcountry_ski",
            name: core.Name,
            geometry: new ActivityGeometryWkt(ActivityGeometryKind.LineString, new WKTWriter().Write(route)),
            iconKey: "backcountry_ski",
            colorHex: "#7A3CCB",
            version: core.Version);

        var events = new DomainEvent[] { created, summary };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(core.Id, events, ct));
        return core.Id;
    }
}
