using System.Threading.Channels;
using Microsoft.Extensions.Logging;

namespace Turbo.Messaging.InProcess;

/// <summary>
/// In-memory backbone of the in-process transport: a bounded channel
/// owned by the publisher side, drained by <see cref="InProcessSubscriberHost"/>.
/// The bounded capacity gives back-pressure to the outbox dispatcher if
/// subscribers fall behind without unboundedly buffering envelopes in the
/// process's heap.
/// </summary>
public sealed class InProcessMessageBus
{
    private readonly Channel<EventEnvelope> _channel;
    private readonly ILogger<InProcessMessageBus> _logger;

    public InProcessMessageBus(ILogger<InProcessMessageBus> logger)
    {
        _channel = Channel.CreateBounded<EventEnvelope>(new BoundedChannelOptions(capacity: 1024)
        {
            FullMode = BoundedChannelFullMode.Wait,
            SingleReader = true,
            SingleWriter = false,
        });
        _logger = logger;
    }

    public ChannelReader<EventEnvelope> Reader => _channel.Reader;

    public async Task PublishAsync(EventEnvelope envelope, CancellationToken cancellationToken)
    {
        await _channel.Writer.WriteAsync(envelope, cancellationToken);
    }
}
