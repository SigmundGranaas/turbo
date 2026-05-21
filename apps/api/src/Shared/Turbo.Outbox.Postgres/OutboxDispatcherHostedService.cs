using System.Text;
using System.Text.Json;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Turbo.Messaging;

namespace Turbo.Outbox.Postgres;

/// <summary>
/// Polls the per-module outbox table, claims a batch under
/// SELECT … FOR UPDATE SKIP LOCKED so multiple instances coexist safely,
/// publishes each row through <see cref="IMessageTransport"/>, and marks
/// the row dispatched. Failed publishes increment <c>attempts</c> and
/// stay undispatched so they get retried on the next tick.
/// </summary>
public sealed class OutboxDispatcherHostedService<TDbContext> : BackgroundService
    where TDbContext : DbContext
{
    private static readonly TimeSpan IdleDelay = TimeSpan.FromMilliseconds(250);
    private static readonly TimeSpan MaxIdleDelay = TimeSpan.FromSeconds(2);
    private const int BatchSize = 100;

    private readonly IServiceScopeFactory _scopeFactory;
    private readonly ILogger<OutboxDispatcherHostedService<TDbContext>> _logger;

    public OutboxDispatcherHostedService(
        IServiceScopeFactory scopeFactory,
        ILogger<OutboxDispatcherHostedService<TDbContext>> logger)
    {
        _scopeFactory = scopeFactory;
        _logger = logger;
    }

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        var idle = IdleDelay;
        while (!stoppingToken.IsCancellationRequested)
        {
            try
            {
                var dispatched = await DispatchBatchAsync(stoppingToken);
                idle = dispatched > 0 ? IdleDelay : SlowDown(idle);
            }
            catch (OperationCanceledException) when (stoppingToken.IsCancellationRequested)
            {
                return;
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Outbox dispatcher batch failed");
                idle = SlowDown(idle);
            }

            try { await Task.Delay(idle, stoppingToken); }
            catch (OperationCanceledException) { return; }
        }
    }

    private async Task<int> DispatchBatchAsync(CancellationToken ct)
    {
        using var scope = _scopeFactory.CreateScope();
        var db = scope.ServiceProvider.GetRequiredService<TDbContext>();
        var transport = scope.ServiceProvider.GetRequiredService<IMessageTransport>();

        var schema = db.Model.FindEntityType(typeof(OutboxRow))?.GetSchema();
        var qualifiedTable = schema is null ? "\"outbox\"" : $"\"{schema}\".\"outbox\"";

        // Run the claim → publish → mark loop inside the configured execution
        // strategy so it composes correctly with EnableRetryOnFailure providers
        // (Npgsql refuses user-initiated transactions otherwise).
        var strategy = db.Database.CreateExecutionStrategy();
        return await strategy.ExecuteAsync(async (innerCt) =>
        {
            await using var tx = await db.Database.BeginTransactionAsync(
                System.Data.IsolationLevel.ReadCommitted, innerCt);

#pragma warning disable EF1002 // schema/table name comes from EF metadata, not user input
            var rows = await db.Set<OutboxRow>()
                .FromSqlRaw($"""
                    SELECT * FROM {qualifiedTable}
                    WHERE dispatched_at IS NULL
                    ORDER BY position
                    LIMIT {BatchSize}
                    FOR UPDATE SKIP LOCKED
                    """)
                .ToListAsync(innerCt);
#pragma warning restore EF1002

            if (rows.Count == 0)
            {
                await tx.CommitAsync(innerCt);
                return 0;
            }

            _logger.LogInformation("Outbox dispatcher claimed {Count} rows from {Table}",
                rows.Count, qualifiedTable);

            var dispatched = 0;
            foreach (var row in rows)
            {
                var envelope = new EventEnvelope(
                    EventId: row.Id,
                    Type: row.EventType,
                    Source: row.Source,
                    Time: row.OccurredAt,
                    DataContentType: row.DataContentType,
                    Data: Encoding.UTF8.GetBytes(row.PayloadJson),
                    Headers: DeserializeHeaders(row.HeadersJson));

                try
                {
                    await transport.PublishAsync(envelope, innerCt);
                    row.DispatchedAt = DateTime.UtcNow;
                    dispatched++;
                    _logger.LogInformation("Outbox dispatched {EventType} {EventId}", row.EventType, row.Id);
                }
                catch (Exception ex)
                {
                    row.Attempts++;
                    row.LastError = ex.Message;
                    _logger.LogWarning(ex,
                        "Outbox publish failed for {EventType} {EventId} (attempt {Attempt})",
                        row.EventType, row.Id, row.Attempts);
                }
            }

            await db.SaveChangesAsync(innerCt);
            await tx.CommitAsync(innerCt);
            return dispatched;
        }, ct);
    }

    private static TimeSpan SlowDown(TimeSpan current)
    {
        var next = TimeSpan.FromMilliseconds(Math.Min(current.TotalMilliseconds * 2, MaxIdleDelay.TotalMilliseconds));
        return next;
    }

    private static IReadOnlyDictionary<string, string> DeserializeHeaders(string json)
    {
        if (string.IsNullOrWhiteSpace(json) || json == "{}")
            return new Dictionary<string, string>();
        return JsonSerializer.Deserialize<Dictionary<string, string>>(json)
               ?? new Dictionary<string, string>();
    }
}
