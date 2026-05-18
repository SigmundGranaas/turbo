using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using NATS.Client.Core;
using NATS.Client.JetStream;
using NATS.Client.JetStream.Models;

namespace Turbo.Messaging.Nats;

/// <summary>
/// Publishes an <see cref="EventEnvelope"/> to a JetStream subject of the
/// form <c>turbo.&lt;source&gt;.&lt;shortType&gt;</c>. Ensures the configured
/// stream exists on first use so tests and fresh deployments do not need a
/// separate provisioning step.
/// </summary>
public sealed class NatsMessageTransport : IMessageTransport, IAsyncDisposable
{
    private readonly NatsConnection _connection;
    private readonly NatsJSContext _js;
    private readonly NatsMessagingOptions _options;
    private readonly ILogger<NatsMessageTransport> _logger;
    private readonly SemaphoreSlim _streamLock = new(1, 1);
    private bool _streamEnsured;

    public NatsMessageTransport(
        IOptions<NatsMessagingOptions> options,
        ILogger<NatsMessageTransport> logger)
    {
        _options = options.Value;
        _logger = logger;
        _connection = new NatsConnection(new NatsOpts { Url = _options.Url });
        _js = new NatsJSContext(_connection);
    }

    public async Task PublishAsync(EventEnvelope envelope, CancellationToken cancellationToken)
    {
        await EnsureStreamAsync(cancellationToken);

        var subject = ToSubject(envelope.Type);
        var headers = new NatsHeaders();
        foreach (var kvp in envelope.Headers)
            headers[kvp.Key] = kvp.Value;
        headers["turbo-event-id"] = envelope.EventId.ToString();
        headers["turbo-event-source"] = envelope.Source;
        headers["turbo-event-time"] = envelope.Time.ToString("O");
        headers["content-type"] = envelope.DataContentType;

        var ack = await _js.PublishAsync(
            subject: subject,
            data: envelope.Data.ToArray(),
            headers: headers,
            cancellationToken: cancellationToken);

        if (ack.Error is not null)
        {
            throw new InvalidOperationException(
                $"NATS publish to {subject} failed: {ack.Error.Description}");
        }
    }

    private async Task EnsureStreamAsync(CancellationToken ct)
    {
        if (_streamEnsured) return;
        await _streamLock.WaitAsync(ct);
        try
        {
            if (_streamEnsured) return;

            var config = new StreamConfig(_options.StreamName, _options.Subjects)
            {
                Retention = StreamConfigRetention.Limits,
                Storage = StreamConfigStorage.File,
                MaxAge = _options.MaxAge,
                NumReplicas = _options.Replicas,
            };
            await _js.CreateOrUpdateStreamAsync(config, ct);
            _streamEnsured = true;
            _logger.LogInformation("Ensured NATS JetStream stream {Stream} on subjects {Subjects}",
                _options.StreamName, string.Join(',', _options.Subjects));
        }
        finally
        {
            _streamLock.Release();
        }
    }

    internal static string ToSubject(string envelopeType)
    {
        // envelope.Type is "turbo.<source>.<shortName>" and matches the
        // configured subject pattern as-is.
        return envelopeType;
    }

    public async ValueTask DisposeAsync()
    {
        _streamLock.Dispose();
        await _connection.DisposeAsync();
    }
}
