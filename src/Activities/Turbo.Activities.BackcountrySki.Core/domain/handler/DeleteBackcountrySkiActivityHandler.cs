using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.BackcountrySki.events;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;

namespace Turboapi.Activities.BackcountrySki.domain.handler;

public sealed class DeleteBackcountrySkiActivityHandler
{
    private readonly IBackcountrySkiActivityReader _reader;
    private readonly IOwnerGuard _ownerGuard;
    private readonly IOutbox<BackcountrySkiScope> _outbox;
    private readonly IUnitOfWork<BackcountrySkiScope> _uow;

    public DeleteBackcountrySkiActivityHandler(
        IBackcountrySkiActivityReader reader,
        IOwnerGuard ownerGuard,
        IOutbox<BackcountrySkiScope> outbox,
        IUnitOfWork<BackcountrySkiScope> uow)
    {
        _reader = reader;
        _ownerGuard = ownerGuard;
        _outbox = outbox;
        _uow = uow;
    }

    public async Task Handle(DeleteBackcountrySkiActivityCommand cmd)
    {
        var existing = await _reader.GetByIdAsync(cmd.ActivityId)
            ?? throw new ActivityNotFoundException(cmd.ActivityId);

        _ownerGuard.RequireOwner(cmd.CallerId, existing.Core.OwnerId);

        if (cmd.IfMatchVersion is { } expected && existing.Core.Version != expected)
            throw new OptimisticConcurrencyException(expected, existing.Core.Version);

        var deleted = new BackcountrySkiActivityDeleted(existing.Core.Id, existing.Core.OwnerId);
        var summaryDelete = new ActivitySummaryDeleted(existing.Core.Id, existing.Core.OwnerId, "backcountry_ski");

        var events = new DomainEvent[] { deleted, summaryDelete };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(existing.Core.Id, events, ct));
    }
}
