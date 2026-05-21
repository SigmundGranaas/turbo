using Microsoft.EntityFrameworkCore;
using Turbo.Messaging;

namespace Turbo.Outbox.Postgres;

/// <summary>
/// EF Core implementation of <see cref="IUnitOfWork{TScope}"/>. Runs the
/// caller's work delegate inside the configured execution strategy and
/// calls SaveChangesAsync at the end so the outbox append + the domain
/// change commit atomically and re-execute idempotently on retry.
///
/// <typeparamref name="TScope"/> is the module marker the handler injects;
/// <typeparamref name="TDbContext"/> is the EF context the module owns
/// — they are decoupled so the handler never has to name the DbContext
/// type.
/// </summary>
public sealed class PgUnitOfWork<TDbContext, TScope> : IUnitOfWork<TScope>
    where TDbContext : DbContext
    where TScope : IModuleScope
{
    private readonly TDbContext _db;

    public PgUnitOfWork(TDbContext db) => _db = db;

    public Task SaveChangesAsync(Func<CancellationToken, Task> work, CancellationToken cancellationToken = default)
        => _db.SaveChangesWithRetryAsync(work, cancellationToken);
}
