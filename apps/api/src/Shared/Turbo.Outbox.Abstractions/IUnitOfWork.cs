using Turbo.Messaging;

namespace Turbo.Outbox;

/// <summary>
/// Commit boundary for a module. Handlers run their stage-changes work
/// inside <see cref="SaveChangesAsync"/>; the implementation owns the
/// underlying transaction and retry semantics (Postgres' execution
/// strategy, in our case).
/// </summary>
public interface IUnitOfWork<TScope>
    where TScope : IModuleScope
{
    Task SaveChangesAsync(Func<CancellationToken, Task> work, CancellationToken cancellationToken = default);
}
