using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.domain;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.Packrafting.events;
using Turboapi.Activities.Packrafting.value;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Packrafting.domain.handler;

public sealed record CreatePackraftingActivityCommand(
    Guid CallerId, string Name, string? Description, string RouteWkt, PackraftingDetails Details);

public sealed record UpdatePackraftingActivityCommand(
    Guid CallerId, Guid ActivityId, string? Name, string? Description, string? RouteWkt, PackraftingDetails? Details)
{
    public long? IfMatchVersion { get; init; }
}

public sealed record DeletePackraftingActivityCommand(Guid CallerId, Guid ActivityId)
{
    public long? IfMatchVersion { get; init; }
}

public interface IPackraftingActivityReader
{
    Task<PackraftingActivity?> GetByIdAsync(Guid activityId, CancellationToken cancellationToken = default);
}

public sealed class CreatePackraftingActivityHandler
{
    private readonly IGeometryNormalizer _geom;
    private readonly IOutbox<PackraftingScope> _outbox;
    private readonly IUnitOfWork<PackraftingScope> _uow;

    public CreatePackraftingActivityHandler(IGeometryNormalizer geom, IOutbox<PackraftingScope> outbox, IUnitOfWork<PackraftingScope> uow)
    { _geom = geom; _outbox = outbox; _uow = uow; }

    public async Task<Guid> Handle(CreatePackraftingActivityCommand cmd)
    {
        var reader = new WKTReader();
        var parsed = reader.Read(cmd.RouteWkt);
        if (parsed.SRID != 4326) parsed.SRID = 4326;
        var route = (LineString)_geom.Normalize(parsed, ActivityGeometryKind.LineString);

        var core = ActivityCore.New(cmd.CallerId, cmd.Name, cmd.Description, route);
        _ = PackraftingActivity.Create(core, cmd.Details);

        var created = new PackraftingActivityCreated(
            core.Id, core.OwnerId, core.Name, core.Description,
            new WKTWriter().Write(route), cmd.Details);

        var summary = new ActivitySummaryUpserted(
            activityId: core.Id, ownerId: core.OwnerId, kind: "packrafting",
            name: core.Name,
            geometry: new ActivityGeometryWkt(ActivityGeometryKind.LineString, new WKTWriter().Write(route)),
            iconKey: "packrafting", colorHex: "#EF6C00", version: core.Version);

        var events = new DomainEvent[] { created, summary };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(core.Id, events, ct));
        return core.Id;
    }
}

public sealed class UpdatePackraftingActivityHandler
{
    private readonly IPackraftingActivityReader _reader;
    private readonly IOwnerGuard _ownerGuard;
    private readonly IGeometryNormalizer _geom;
    private readonly IOutbox<PackraftingScope> _outbox;
    private readonly IUnitOfWork<PackraftingScope> _uow;

    public UpdatePackraftingActivityHandler(IPackraftingActivityReader reader, IOwnerGuard ownerGuard, IGeometryNormalizer geom,
        IOutbox<PackraftingScope> outbox, IUnitOfWork<PackraftingScope> uow)
    { _reader = reader; _ownerGuard = ownerGuard; _geom = geom; _outbox = outbox; _uow = uow; }

    public async Task Handle(UpdatePackraftingActivityCommand cmd)
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

        var updated = new PackraftingActivityUpdated(
            next.Core.Id, next.Core.OwnerId, cmd.Name, cmd.Description, cmd.RouteWkt, cmd.Details, next.Core.Version);
        var summary = new ActivitySummaryUpserted(
            activityId: next.Core.Id, ownerId: next.Core.OwnerId, kind: "packrafting",
            name: next.Core.Name,
            geometry: new ActivityGeometryWkt(ActivityGeometryKind.LineString, new WKTWriter().Write(next.Core.Geometry)),
            iconKey: "packrafting", colorHex: "#EF6C00", version: next.Core.Version);
        var events = new DomainEvent[] { updated, summary };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(next.Core.Id, events, ct));
    }
}

public sealed class DeletePackraftingActivityHandler
{
    private readonly IPackraftingActivityReader _reader;
    private readonly IOwnerGuard _ownerGuard;
    private readonly IOutbox<PackraftingScope> _outbox;
    private readonly IUnitOfWork<PackraftingScope> _uow;

    public DeletePackraftingActivityHandler(IPackraftingActivityReader reader, IOwnerGuard ownerGuard,
        IOutbox<PackraftingScope> outbox, IUnitOfWork<PackraftingScope> uow)
    { _reader = reader; _ownerGuard = ownerGuard; _outbox = outbox; _uow = uow; }

    public async Task Handle(DeletePackraftingActivityCommand cmd)
    {
        var existing = await ReadModelCatchup.ReadAsync(
                ct => _reader.GetByIdAsync(cmd.ActivityId, ct))
            ?? throw new ActivityNotFoundException(cmd.ActivityId);
        _ownerGuard.RequireOwner(cmd.CallerId, existing.Core.OwnerId);

        if (cmd.IfMatchVersion is { } expected && existing.Core.Version != expected)
            throw new OptimisticConcurrencyException(expected, existing.Core.Version);

        var deleted = new PackraftingActivityDeleted(existing.Core.Id, existing.Core.OwnerId);
        var summaryDelete = new ActivitySummaryDeleted(existing.Core.Id, existing.Core.OwnerId, "packrafting");
        var events = new DomainEvent[] { deleted, summaryDelete };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(existing.Core.Id, events, ct));
    }
}
