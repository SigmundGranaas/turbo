using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.Hiking.events;

namespace Turboapi.Activities.Hiking.domain.handler;

public sealed class DeleteHikingActivityHandler
{
    private readonly IHikingActivityReader _reader;
    private readonly IOwnerGuard _ownerGuard;
    private readonly IOutbox<HikingScope> _outbox;
    private readonly IUnitOfWork<HikingScope> _uow;

    public DeleteHikingActivityHandler(
        IHikingActivityReader reader, IOwnerGuard ownerGuard,
        IOutbox<HikingScope> outbox, IUnitOfWork<HikingScope> uow)
    {
        _reader = reader; _ownerGuard = ownerGuard; _outbox = outbox; _uow = uow;
    }

    public async Task Handle(DeleteHikingActivityCommand cmd)
    {
        var existing = await ReadModelCatchup.ReadAsync(
                ct => _reader.GetByIdAsync(cmd.ActivityId, ct))
            ?? throw new ActivityNotFoundException(cmd.ActivityId);
        _ownerGuard.RequireOwner(cmd.CallerId, existing.Core.OwnerId);

        if (cmd.IfMatchVersion is { } expected && existing.Core.Version != expected)
            throw new OptimisticConcurrencyException(expected, existing.Core.Version);

        var deleted = new HikingActivityDeleted(existing.Core.Id, existing.Core.OwnerId);
        var summaryDelete = new ActivitySummaryDeleted(existing.Core.Id, existing.Core.OwnerId, "hiking");
        var events = new DomainEvent[] { deleted, summaryDelete };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(existing.Core.Id, events, ct));
    }
}
