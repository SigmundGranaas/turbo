namespace Turbo.Outbox.Postgres;

public sealed class OutboxRow
{
    public Guid Id { get; set; }
    public Guid AggregateId { get; set; }
    public string EventType { get; set; } = string.Empty;
    public string Source { get; set; } = string.Empty;
    public string DataContentType { get; set; } = "application/json";
    public string PayloadJson { get; set; } = "{}";
    public string HeadersJson { get; set; } = "{}";
    public DateTime OccurredAt { get; set; }
    public long Position { get; set; }
    public DateTime? DispatchedAt { get; set; }
    public int Attempts { get; set; }
    public string? LastError { get; set; }
}
