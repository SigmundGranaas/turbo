using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.domain;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.Freediving.events;
using Turboapi.Activities.Freediving.value;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Freediving.domain.handler;

public sealed record CreateFreedivingActivityCommand(
    Guid CallerId, string Name, string? Description,
    double Longitude, double Latitude, FreedivingDetails Details);

public sealed record UpdateFreedivingActivityCommand(
    Guid CallerId, Guid ActivityId, string? Name, string? Description,
    double? Longitude, double? Latitude, FreedivingDetails? Details)
{
    public long? IfMatchVersion { get; init; }
}

public sealed record DeleteFreedivingActivityCommand(Guid CallerId, Guid ActivityId)
{
    public long? IfMatchVersion { get; init; }
}

public interface IFreedivingActivityReader
{
    Task<FreedivingActivity?> GetByIdAsync(Guid activityId, CancellationToken cancellationToken = default);
}

public sealed class CreateFreedivingActivityHandler
{
    private readonly IGeometryNormalizer _geom;
    private readonly IOutbox<FreedivingScope> _outbox;
    private readonly IUnitOfWork<FreedivingScope> _uow;

    public CreateFreedivingActivityHandler(IGeometryNormalizer geom, IOutbox<FreedivingScope> outbox, IUnitOfWork<FreedivingScope> uow)
    { _geom = geom; _outbox = outbox; _uow = uow; }

    public async Task<Guid> Handle(CreateFreedivingActivityCommand cmd)
    {
        var factory = new GeometryFactory(new PrecisionModel(), 4326);
        var point = factory.CreatePoint(new Coordinate(cmd.Longitude, cmd.Latitude));
        var normalized = _geom.Normalize(point, ActivityGeometryKind.Point);

        var core = ActivityCore.New(cmd.CallerId, cmd.Name, cmd.Description, normalized);
        _ = FreedivingActivity.Create(core, cmd.Details);

        var created = new FreedivingActivityCreated(
            core.Id, core.OwnerId, core.Name, core.Description,
            cmd.Longitude, cmd.Latitude, cmd.Details);

        var summary = new ActivitySummaryUpserted(
            activityId: core.Id, ownerId: core.OwnerId, kind: "freediving",
            name: core.Name,
            geometry: new ActivityGeometryWkt(ActivityGeometryKind.Point, new WKTWriter().Write(normalized)),
            iconKey: "freediving", colorHex: "#1565C0", version: core.Version);

        var events = new DomainEvent[] { created, summary };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(core.Id, events, ct));
        return core.Id;
    }
}

public sealed class UpdateFreedivingActivityHandler
{
    private readonly IFreedivingActivityReader _reader;
    private readonly IOwnerGuard _ownerGuard;
    private readonly IGeometryNormalizer _geom;
    private readonly IOutbox<FreedivingScope> _outbox;
    private readonly IUnitOfWork<FreedivingScope> _uow;

    public UpdateFreedivingActivityHandler(IFreedivingActivityReader reader, IOwnerGuard ownerGuard, IGeometryNormalizer geom,
        IOutbox<FreedivingScope> outbox, IUnitOfWork<FreedivingScope> uow)
    { _reader = reader; _ownerGuard = ownerGuard; _geom = geom; _outbox = outbox; _uow = uow; }

    public async Task Handle(UpdateFreedivingActivityCommand cmd)
    {
        var existing = await _reader.GetByIdAsync(cmd.ActivityId)
            ?? throw new ActivityNotFoundException(cmd.ActivityId);
        _ownerGuard.RequireOwner(cmd.CallerId, existing.Core.OwnerId);

        if (cmd.IfMatchVersion is { } expected && existing.Core.Version != expected)
            throw new OptimisticConcurrencyException(expected, existing.Core.Version);

        var next = existing;
        var changed = false;
        if (cmd.Name is not null || cmd.Description is not null) { next = next.Rename(cmd.Name, cmd.Description); changed = true; }
        if (cmd.Longitude is not null && cmd.Latitude is not null)
        {
            var factory = new GeometryFactory(new PrecisionModel(), 4326);
            var p = factory.CreatePoint(new Coordinate(cmd.Longitude.Value, cmd.Latitude.Value));
            next = next.Relocate((Point)_geom.Normalize(p, ActivityGeometryKind.Point));
            changed = true;
        }
        if (cmd.Details is not null) { next = next.ReplaceDetails(cmd.Details); changed = true; }
        if (!changed) return;

        var updated = new FreedivingActivityUpdated(
            next.Core.Id, next.Core.OwnerId, cmd.Name, cmd.Description,
            cmd.Longitude, cmd.Latitude, cmd.Details, next.Core.Version);
        var summary = new ActivitySummaryUpserted(
            activityId: next.Core.Id, ownerId: next.Core.OwnerId, kind: "freediving",
            name: next.Core.Name,
            geometry: new ActivityGeometryWkt(ActivityGeometryKind.Point, new WKTWriter().Write(next.Core.Geometry)),
            iconKey: "freediving", colorHex: "#1565C0", version: next.Core.Version);
        var events = new DomainEvent[] { updated, summary };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(next.Core.Id, events, ct));
    }
}

public sealed class DeleteFreedivingActivityHandler
{
    private readonly IFreedivingActivityReader _reader;
    private readonly IOwnerGuard _ownerGuard;
    private readonly IOutbox<FreedivingScope> _outbox;
    private readonly IUnitOfWork<FreedivingScope> _uow;

    public DeleteFreedivingActivityHandler(IFreedivingActivityReader reader, IOwnerGuard ownerGuard,
        IOutbox<FreedivingScope> outbox, IUnitOfWork<FreedivingScope> uow)
    { _reader = reader; _ownerGuard = ownerGuard; _outbox = outbox; _uow = uow; }

    public async Task Handle(DeleteFreedivingActivityCommand cmd)
    {
        var existing = await _reader.GetByIdAsync(cmd.ActivityId)
            ?? throw new ActivityNotFoundException(cmd.ActivityId);
        _ownerGuard.RequireOwner(cmd.CallerId, existing.Core.OwnerId);

        if (cmd.IfMatchVersion is { } expected && existing.Core.Version != expected)
            throw new OptimisticConcurrencyException(expected, existing.Core.Version);

        var deleted = new FreedivingActivityDeleted(existing.Core.Id, existing.Core.OwnerId);
        var summaryDelete = new ActivitySummaryDeleted(existing.Core.Id, existing.Core.OwnerId, "freediving");
        var events = new DomainEvent[] { deleted, summaryDelete };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(existing.Core.Id, events, ct));
    }
}
