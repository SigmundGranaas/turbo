using Microsoft.EntityFrameworkCore;

namespace Turbo.Outbox.Postgres;

public static class OutboxModelExtensions
{
    /// <summary>
    /// Map an <see cref="OutboxRow"/> entity into a module's DbContext.
    /// Call from <c>OnModelCreating</c>. The schema name (e.g. "auth",
    /// "activity", "geo") keeps each module's outbox in its own namespace
    /// when modules share a single Postgres instance in modulith mode.
    /// </summary>
    public static ModelBuilder MapOutbox(this ModelBuilder modelBuilder, string? schema = null)
    {
        modelBuilder.Entity<OutboxRow>(b =>
        {
            if (schema is not null) b.ToTable("outbox", schema);
            else b.ToTable("outbox");

            b.HasKey(x => x.Id);
            b.Property(x => x.Position).ValueGeneratedOnAdd();
            b.Property(x => x.PayloadJson).HasColumnType("jsonb");
            b.Property(x => x.HeadersJson).HasColumnType("jsonb");
            b.HasIndex(x => new { x.DispatchedAt, x.Position })
                .HasDatabaseName("outbox_undispatched")
                .HasFilter("dispatched_at IS NULL");
            b.HasIndex(x => new { x.AggregateId, x.Position })
                .HasDatabaseName("outbox_aggregate");

            b.Property(x => x.Id).HasColumnName("id");
            b.Property(x => x.AggregateId).HasColumnName("aggregate_id");
            b.Property(x => x.EventType).HasColumnName("event_type");
            b.Property(x => x.Source).HasColumnName("source");
            b.Property(x => x.DataContentType).HasColumnName("data_content_type");
            b.Property(x => x.PayloadJson).HasColumnName("payload_json");
            b.Property(x => x.HeadersJson).HasColumnName("headers_json");
            b.Property(x => x.OccurredAt).HasColumnName("occurred_at");
            b.Property(x => x.Position).HasColumnName("position");
            b.Property(x => x.DispatchedAt).HasColumnName("dispatched_at");
            b.Property(x => x.Attempts).HasColumnName("attempts");
            b.Property(x => x.LastError).HasColumnName("last_error");
        });
        return modelBuilder;
    }

    /// <summary>
    /// Map a <see cref="ProcessedEventRow"/> entity into a module's DbContext
    /// alongside <see cref="OutboxRow"/>. Call from <c>OnModelCreating</c>.
    /// The schema name is the same one <see cref="MapOutbox"/> uses so each
    /// module's idempotency table stays in its own namespace.
    /// </summary>
    public static ModelBuilder MapProcessedEvents(this ModelBuilder modelBuilder, string? schema = null)
    {
        modelBuilder.Entity<ProcessedEventRow>(b =>
        {
            if (schema is not null) b.ToTable("processed_events", schema);
            else b.ToTable("processed_events");

            b.HasKey(x => x.EventId);
            b.Property(x => x.EventId).HasColumnName("event_id");
            // PgIdempotencyStore writes only event_id and relies on the DB
            // to stamp processed_at via CURRENT_TIMESTAMP, so the column
            // must have a default. Without this, the INSERT
            // (event_id) VALUES (...) ON CONFLICT DO NOTHING fails with
            // a NOT NULL constraint violation on processed_at.
            b.Property(x => x.ProcessedAt)
                .HasColumnName("processed_at")
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd();
        });
        return modelBuilder;
    }
}
