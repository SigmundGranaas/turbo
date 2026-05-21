using Turbo.Messaging;

namespace Turbo.Outbox;

/// <summary>
/// Append-only queue of events that a module wants to publish. The contract is
/// that <see cref="AppendAsync"/> participates in the caller's ambient
/// database transaction — so committing the aggregate change and committing
/// the event row happen together or not at all.
///
/// The <typeparamref name="TScope"/> module marker carries the module's
/// source name as a static abstract property (see <see cref="IModuleScope"/>)
/// so handlers never need to pass it as a magic string, and DI resolution
/// keeps each module's outbox separate when multiple modules share a
/// process.
///
/// Modules (or their UnitOfWork) call this; an out-of-band dispatcher
/// hosted service is responsible for moving the rows to a transport.
/// </summary>
public interface IOutbox<TScope>
    where TScope : IModuleScope
{
    Task AppendAsync(EventEnvelope envelope, CancellationToken cancellationToken);
}
