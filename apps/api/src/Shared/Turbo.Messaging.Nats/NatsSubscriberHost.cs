using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using NATS.Client.Core;
using NATS.Client.JetStream;
using NATS.Client.JetStream.Models;

namespace Turbo.Messaging.Nats;

/// <summary>
/// Hosted service that binds one JetStream durable consumer per
/// <see cref="NatsSubscriberRegistration"/> and dispatches received
/// messages to the corresponding <c>IEventHandler&lt;TEvent&gt;</c> in a fresh
/// DI scope. At-least-once delivery; the handler must be idempotent.
/// </summary>
public sealed class NatsSubscriberHost : BackgroundService
{
    private readonly IServiceScopeFactory _scopeFactory;
    private readonly IEnumerable<NatsSubscriberRegistration> _registrations;
    private readonly NatsMessagingOptions _options;
    private readonly ILogger<NatsSubscriberHost> _logger;
    private NatsConnection? _connection;
    private NatsJSContext? _js;

    public NatsSubscriberHost(
        IServiceScopeFactory scopeFactory,
        IEnumerable<NatsSubscriberRegistration> registrations,
        IOptions<NatsMessagingOptions> options,
        ILogger<NatsSubscriberHost> logger)
    {
        _scopeFactory = scopeFactory;
        _registrations = registrations;
        _options = options.Value;
        _logger = logger;
    }

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        _connection = new NatsConnection(new NatsOpts { Url = _options.Url });
        _js = new NatsJSContext(_connection);

        await EnsureStreamAsync(stoppingToken);

        var tasks = _registrations
            .Select(reg => Task.Run(() => ConsumeAsync(reg, stoppingToken), stoppingToken))
            .ToArray();
        await Task.WhenAll(tasks);
    }

    private async Task EnsureStreamAsync(CancellationToken ct)
    {
        var config = new StreamConfig(_options.StreamName, _options.Subjects)
        {
            Retention = StreamConfigRetention.Limits,
            Storage = StreamConfigStorage.File,
            MaxAge = _options.MaxAge,
            NumReplicas = _options.Replicas,
        };
        await _js!.CreateOrUpdateStreamAsync(config, ct);
    }

    private async Task ConsumeAsync(NatsSubscriberRegistration reg, CancellationToken ct)
    {
        var consumerConfig = new ConsumerConfig(reg.DurableName)
        {
            FilterSubject = reg.Subject,
            AckPolicy = ConsumerConfigAckPolicy.Explicit,
            DeliverPolicy = ConsumerConfigDeliverPolicy.All,
            MaxDeliver = 10,
            AckWait = TimeSpan.FromSeconds(30),
        };

        // Cross-service subscribers (Sharing reading from TURBO_COLLECTIONS /
        // TURBO_GEO / TURBO_TRACKS) override the host's default stream per
        // registration. Same-service subscribers fall through to the default.
        var stream = reg.StreamName ?? _options.StreamName;

        INatsJSConsumer consumer;
        try
        {
            consumer = await _js!.CreateOrUpdateConsumerAsync(
                stream, consumerConfig, ct);
        }
        catch (Exception ex)
        {
            // Cross-service subscriber whose peer stream isn't published yet.
            // Log + return — the peer service can come up later and a host
            // restart will pick up the events from the stream's history.
            _logger.LogWarning(ex,
                "NATS subscriber could not bind to stream={Stream} subject={Subject}; skipping",
                stream, reg.Subject);
            return;
        }

        _logger.LogInformation(
            "NATS subscriber bound: stream={Stream} durable={Durable} subject={Subject} eventType={Event}",
            stream, reg.DurableName, reg.Subject, reg.EventType.Name);

        await foreach (var msg in consumer.ConsumeAsync<byte[]>(cancellationToken: ct))
        {
            try
            {
                await using var scope = _scopeFactory.CreateAsyncScope();
                if (msg.Data is null)
                {
                    await msg.AckAsync(cancellationToken: ct);
                    continue;
                }
                await reg.Dispatcher(scope.ServiceProvider, msg.Data, ct);
                await msg.AckAsync(cancellationToken: ct);
            }
            catch (Exception ex)
            {
                _logger.LogWarning(ex,
                    "NATS subscriber failed to dispatch {Subject} — NAK and retry",
                    reg.Subject);
                try { await msg.NakAsync(cancellationToken: ct); }
                catch { /* connection may have dropped — let the loop handle it */ }
            }
        }
    }

    public override async Task StopAsync(CancellationToken cancellationToken)
    {
        await base.StopAsync(cancellationToken);
        if (_connection is not null) await _connection.DisposeAsync();
    }
}
