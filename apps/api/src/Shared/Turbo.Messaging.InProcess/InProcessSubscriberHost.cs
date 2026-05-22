using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;

namespace Turbo.Messaging.InProcess;

/// <summary>
/// Reads envelopes out of <see cref="InProcessMessageBus"/> and routes
/// them to the registered <see cref="IEventHandler{TEvent}"/> via a fresh
/// DI scope per delivery. At-least-once delivery; handlers are expected
/// to be idempotent on retry.
///
/// If a handler throws the envelope is re-published to the back of the
/// channel with an attempt counter; this keeps the in-process flow
/// symmetric with the NATS path (handler throws → NAK → redelivery).
/// </summary>
public sealed class InProcessSubscriberHost : BackgroundService
{
    private const int MaxRedeliveries = 5;

    private readonly InProcessMessageBus _bus;
    private readonly IServiceScopeFactory _scopeFactory;
    private readonly IReadOnlyDictionary<string, InProcessSubscriberRegistration> _bySubject;
    private readonly ILogger<InProcessSubscriberHost> _logger;

    public InProcessSubscriberHost(
        InProcessMessageBus bus,
        IServiceScopeFactory scopeFactory,
        IEnumerable<InProcessSubscriberRegistration> registrations,
        ILogger<InProcessSubscriberHost> logger)
    {
        _bus = bus;
        _scopeFactory = scopeFactory;
        _bySubject = registrations.ToDictionary(r => r.Subject, r => r);
        _logger = logger;
    }

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        _logger.LogInformation(
            "InProcessSubscriberHost started for {Count} subjects: {Subjects}",
            _bySubject.Count, string.Join(", ", _bySubject.Keys));

        await foreach (var envelope in _bus.Reader.ReadAllAsync(stoppingToken))
        {
            await DispatchAsync(envelope, stoppingToken);
        }
    }

    private async Task DispatchAsync(EventEnvelope envelope, CancellationToken ct)
    {
        if (!_bySubject.TryGetValue(envelope.Type, out var registration))
        {
            // No subscriber for this subject in this process. Audit log
            // and drop — the outbox row is already marked dispatched.
            _logger.LogDebug("No in-process subscriber for {Subject}", envelope.Type);
            return;
        }

        var attempt = 0;
        envelope.Headers.TryGetValue("turbo-redeliveries", out var redeliveriesHeader);
        int.TryParse(redeliveriesHeader, out attempt);

        try
        {
            await using var scope = _scopeFactory.CreateAsyncScope();
            await registration.Dispatcher(scope.ServiceProvider, envelope.Data, ct);
        }
        catch (Exception ex) when (attempt < MaxRedeliveries)
        {
            _logger.LogWarning(ex,
                "In-process handler for {Subject} threw on attempt {Attempt}; redelivering",
                envelope.Type, attempt + 1);
            var retryHeaders = new Dictionary<string, string>(envelope.Headers)
            {
                ["turbo-redeliveries"] = (attempt + 1).ToString(),
            };
            var retry = envelope with { Headers = retryHeaders };
            await _bus.PublishAsync(retry, ct);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex,
                "In-process handler for {Subject} gave up after {Attempts} attempts; dropping envelope {EventId}",
                envelope.Type, attempt, envelope.EventId);
        }
    }
}
