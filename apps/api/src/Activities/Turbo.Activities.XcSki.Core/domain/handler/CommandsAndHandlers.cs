using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.domain;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.value;
using Turboapi.Activities.XcSki.events;
using Turboapi.Activities.XcSki.value;

namespace Turboapi.Activities.XcSki.domain.handler;

public sealed record CreateXcSkiActivityCommand(
    Guid CallerId, string Name, string? Description, string RouteWkt, XcSkiDetails Details);

public sealed record UpdateXcSkiActivityCommand(
    Guid CallerId, Guid ActivityId, string? Name, string? Description, string? RouteWkt, XcSkiDetails? Details)
{
    public long? IfMatchVersion { get; init; }
}

public sealed record DeleteXcSkiActivityCommand(Guid CallerId, Guid ActivityId)
{
    public long? IfMatchVersion { get; init; }
}

public interface IXcSkiActivityReader
{
    Task<XcSkiActivity?> GetByIdAsync(Guid activityId, CancellationToken cancellationToken = default);
}

public sealed class CreateXcSkiActivityHandler
{
    private readonly IGeometryNormalizer _geom;
    private readonly IOutbox<XcSkiScope> _outbox;
    private readonly IUnitOfWork<XcSkiScope> _uow;

    public CreateXcSkiActivityHandler(IGeometryNormalizer geom, IOutbox<XcSkiScope> outbox, IUnitOfWork<XcSkiScope> uow)
    { _geom = geom; _outbox = outbox; _uow = uow; }

    public async Task<Guid> Handle(CreateXcSkiActivityCommand cmd)
    {
        var reader = new WKTReader();
        var parsed = reader.Read(cmd.RouteWkt);
        if (parsed.SRID != 4326) parsed.SRID = 4326;
        var route = (LineString)_geom.Normalize(parsed, ActivityGeometryKind.LineString);

        var core = ActivityCore.New(cmd.CallerId, cmd.Name, cmd.Description, route);
        _ = XcSkiActivity.Create(core, cmd.Details);

        var created = new XcSkiActivityCreated(
            core.Id, core.OwnerId, core.Name, core.Description,
            new WKTWriter().Write(route), cmd.Details);

        var summary = new ActivitySummaryUpserted(
            activityId: core.Id, ownerId: core.OwnerId, kind: "xc_ski",
            name: core.Name,
            geometry: new ActivityGeometryWkt(ActivityGeometryKind.LineString, new WKTWriter().Write(route)),
            iconKey: "xc_ski", colorHex: "#00838F", version: core.Version);

        var events = new DomainEvent[] { created, summary };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(core.Id, events, ct));
        return core.Id;
    }
}

public sealed class UpdateXcSkiActivityHandler
{
    private readonly IXcSkiActivityReader _reader;
    private readonly IOwnerGuard _ownerGuard;
    private readonly IGeometryNormalizer _geom;
    private readonly IOutbox<XcSkiScope> _outbox;
    private readonly IUnitOfWork<XcSkiScope> _uow;

    public UpdateXcSkiActivityHandler(IXcSkiActivityReader reader, IOwnerGuard ownerGuard, IGeometryNormalizer geom,
        IOutbox<XcSkiScope> outbox, IUnitOfWork<XcSkiScope> uow)
    { _reader = reader; _ownerGuard = ownerGuard; _geom = geom; _outbox = outbox; _uow = uow; }

    public async Task Handle(UpdateXcSkiActivityCommand cmd)
    {
        var existing = await ReadModelCatchup.ReadAsync(
                ct => _reader.GetByIdAsync(cmd.ActivityId, ct))
            ?? throw new ActivityNotFoundException(cmd.ActivityId);
        _ownerGuard.RequireOwner(cmd.CallerId, existing.Core.OwnerId);

        if (cmd.IfMatchVersion is { } expected && existing.Core.Version != expected)
            throw new OptimisticConcurrencyException(expected, existing.Core.Version);

        var next = existing;
        var changed = false;
        if (cmd.Name is not null || cmd.Description is not null) { next = next.Rename(cmd.Name, cmd.Description); changed = true; }
        if (cmd.RouteWkt is not null)
        {
            var reader = new WKTReader();
            var parsed = reader.Read(cmd.RouteWkt);
            if (parsed.SRID != 4326) parsed.SRID = 4326;
            var route = (LineString)_geom.Normalize(parsed, ActivityGeometryKind.LineString);
            next = next.ReplaceRoute(route);
            changed = true;
        }
        if (cmd.Details is not null) { next = next.ReplaceDetails(cmd.Details); changed = true; }
        if (!changed) return;

        var updated = new XcSkiActivityUpdated(
            next.Core.Id, next.Core.OwnerId, cmd.Name, cmd.Description, cmd.RouteWkt, cmd.Details, next.Core.Version);
        var summary = new ActivitySummaryUpserted(
            activityId: next.Core.Id, ownerId: next.Core.OwnerId, kind: "xc_ski",
            name: next.Core.Name,
            geometry: new ActivityGeometryWkt(ActivityGeometryKind.LineString, new WKTWriter().Write(next.Core.Geometry)),
            iconKey: "xc_ski", colorHex: "#00838F", version: next.Core.Version);
        var events = new DomainEvent[] { updated, summary };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(next.Core.Id, events, ct));
    }
}

public sealed class DeleteXcSkiActivityHandler
{
    private readonly IXcSkiActivityReader _reader;
    private readonly IOwnerGuard _ownerGuard;
    private readonly IOutbox<XcSkiScope> _outbox;
    private readonly IUnitOfWork<XcSkiScope> _uow;

    public DeleteXcSkiActivityHandler(IXcSkiActivityReader reader, IOwnerGuard ownerGuard,
        IOutbox<XcSkiScope> outbox, IUnitOfWork<XcSkiScope> uow)
    { _reader = reader; _ownerGuard = ownerGuard; _outbox = outbox; _uow = uow; }

    public async Task Handle(DeleteXcSkiActivityCommand cmd)
    {
        var existing = await ReadModelCatchup.ReadAsync(
                ct => _reader.GetByIdAsync(cmd.ActivityId, ct))
            ?? throw new ActivityNotFoundException(cmd.ActivityId);
        _ownerGuard.RequireOwner(cmd.CallerId, existing.Core.OwnerId);

        if (cmd.IfMatchVersion is { } expected && existing.Core.Version != expected)
            throw new OptimisticConcurrencyException(expected, existing.Core.Version);

        var deleted = new XcSkiActivityDeleted(existing.Core.Id, existing.Core.OwnerId);
        var summaryDelete = new ActivitySummaryDeleted(existing.Core.Id, existing.Core.OwnerId, "xc_ski");
        var events = new DomainEvent[] { deleted, summaryDelete };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(existing.Core.Id, events, ct));
    }
}
