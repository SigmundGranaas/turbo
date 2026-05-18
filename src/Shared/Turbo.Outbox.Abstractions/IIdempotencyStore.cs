namespace Turbo.Outbox;

/// <summary>
/// Per-module dedup table. At-least-once delivery means a single envelope
/// can arrive at a subscriber more than once — on broker retry, on
/// rebalance, or in the in-process bus's redelivery loop. Projection
/// handlers wrap their work in <see cref="TryMarkProcessedAsync"/>:
///
///   if (await store.TryMarkProcessedAsync(envelope.EventId, ct))
///   {
///       // first time we see this event_id → do the projection
///   }
///   // already processed → skip
///
/// The <typeparamref name="TDbContext"/> marker keeps each module's store
/// independent in a modulith where three of them share the process.
/// </summary>
public interface IIdempotencyStore<TDbContext>
{
    /// <returns>
    /// <c>true</c> if this is the first time the event id has been seen
    /// (the caller should proceed with the projection),
    /// <c>false</c> if a previous delivery already marked it processed.
    /// </returns>
    Task<bool> TryMarkProcessedAsync(Guid eventId, CancellationToken cancellationToken);
}
