using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.Fishing.events;

namespace Turboapi.Activities.Fishing.domain.handler;

public sealed class DeleteFishingActivityHandler
{
    private readonly IFishingActivityReader _reader;
    private readonly IOwnerGuard _ownerGuard;
    private readonly IOutbox<FishingScope> _outbox;
    private readonly IUnitOfWork<FishingScope> _uow;

    public DeleteFishingActivityHandler(
        IFishingActivityReader reader,
        IOwnerGuard ownerGuard,
        IOutbox<FishingScope> outbox,
        IUnitOfWork<FishingScope> uow)
    {
        _reader = reader;
        _ownerGuard = ownerGuard;
        _outbox = outbox;
        _uow = uow;
    }

    public async Task Handle(DeleteFishingActivityCommand cmd)
    {
        var existing = await ReadModelCatchup.ReadAsync(
                ct => _reader.GetByIdAsync(cmd.ActivityId, ct))
            ?? throw new ActivityNotFoundException(cmd.ActivityId);

        _ownerGuard.RequireOwner(cmd.CallerId, existing.Core.OwnerId);

        if (cmd.IfMatchVersion is { } expected && existing.Core.Version != expected)
            throw new OptimisticConcurrencyException(expected, existing.Core.Version);

        var deleted = new FishingActivityDeleted(existing.Core.Id, existing.Core.OwnerId);
        var summaryDelete = new ActivitySummaryDeleted(existing.Core.Id, existing.Core.OwnerId, "fishing");

        var events = new DomainEvent[] { deleted, summaryDelete };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(existing.Core.Id, events, ct));
    }
}
