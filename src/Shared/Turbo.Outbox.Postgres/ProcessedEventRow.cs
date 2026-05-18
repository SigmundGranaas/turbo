namespace Turbo.Outbox.Postgres;

/// <summary>
/// One row per delivered event id, written by the idempotency store
/// before the projection. The primary key is the event id, so a second
/// arrival of the same id collides and the projection skips.
/// </summary>
public sealed class ProcessedEventRow
{
    public Guid EventId { get; set; }
    public DateTime ProcessedAt { get; set; }
}
