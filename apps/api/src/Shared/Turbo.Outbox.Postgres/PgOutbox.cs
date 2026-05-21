using System.Text.Json;
using Microsoft.EntityFrameworkCore;
using Turbo.Messaging;

namespace Turbo.Outbox.Postgres;

/// <summary>
/// EF Core-backed <see cref="IOutbox{TScope}"/>. Inserting an
/// <see cref="OutboxRow"/> through the same <typeparamref name="TDbContext"/>
/// the aggregate is written against ensures the event row commits
/// atomically with the domain change.
///
/// <typeparamref name="TScope"/> is the module marker the handler injects;
/// <typeparamref name="TDbContext"/> is the EF context the module owns.
/// They are decoupled so the handler never has to name the DbContext type.
/// </summary>
public sealed class PgOutbox<TDbContext, TScope> : IOutbox<TScope>
    where TDbContext : DbContext
    where TScope : IModuleScope
{
    private readonly TDbContext _db;

    public PgOutbox(TDbContext db) => _db = db;

    public async Task AppendAsync(EventEnvelope envelope, CancellationToken cancellationToken)
    {
        var row = new OutboxRow
        {
            Id = envelope.EventId == Guid.Empty ? Guid.NewGuid() : envelope.EventId,
            AggregateId = TryReadAggregateId(envelope.Headers),
            EventType = envelope.Type,
            Source = envelope.Source,
            DataContentType = envelope.DataContentType,
            PayloadJson = System.Text.Encoding.UTF8.GetString(envelope.Data.Span),
            HeadersJson = JsonSerializer.Serialize(envelope.Headers),
            OccurredAt = envelope.Time,
        };
        await _db.Set<OutboxRow>().AddAsync(row, cancellationToken);
    }

    private static Guid TryReadAggregateId(IReadOnlyDictionary<string, string> headers)
    {
        return headers.TryGetValue("aggregateId", out var raw) && Guid.TryParse(raw, out var g)
            ? g
            : Guid.Empty;
    }
}
