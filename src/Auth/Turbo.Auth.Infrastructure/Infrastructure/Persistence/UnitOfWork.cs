using Turbo.Messaging;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Auth.Domain;

namespace Turboapi.Auth.Infrastructure.Persistence
{
    /// <summary>
    /// Auth's <see cref="IUnitOfWork{TScope}"/>. Wraps the DbContext's
    /// SaveChanges in the configured execution strategy AND drains
    /// every tracked aggregate's <see cref="IHasDomainEvents.DomainEvents"/>
    /// into the outbox atomically with the aggregate save. The decorator
    /// pattern in <c>UnitOfWorkCommandHandlerDecorator</c> invokes this
    /// with an empty work delegate after a successful handler call —
    /// handlers stage changes through repositories, this commits them.
    /// </summary>
    public sealed class AuthUnitOfWork : IUnitOfWork<AuthScope>
    {
        private readonly AuthDbContext _dbContext;
        private readonly IOutbox<AuthScope> _outbox;

        public AuthUnitOfWork(AuthDbContext dbContext, IOutbox<AuthScope> outbox)
        {
            _dbContext = dbContext ?? throw new ArgumentNullException(nameof(dbContext));
            _outbox = outbox ?? throw new ArgumentNullException(nameof(outbox));
        }

        public Task SaveChangesAsync(Func<CancellationToken, Task> work, CancellationToken cancellationToken = default)
            => _dbContext.SaveChangesWithRetryAsync(async ct =>
            {
                await work(ct);
                await DrainDomainEventsToOutboxAsync(ct);
            }, cancellationToken);

        private async Task DrainDomainEventsToOutboxAsync(CancellationToken cancellationToken)
        {
            var aggregates = _dbContext.ChangeTracker.Entries<IHasDomainEvents>()
                .Select(e => e.Entity)
                .Where(a => a.DomainEvents.Count > 0)
                .ToList();

            foreach (var aggregate in aggregates)
            {
                var headers = new Dictionary<string, string>();
                if (aggregate is Turboapi.Auth.Domain.Aggregates.Account account)
                    headers["aggregateId"] = account.Id.ToString();

                var pending = aggregate.DomainEvents.ToList();
                aggregate.ClearDomainEvents();
                foreach (var @event in pending)
                {
                    var envelope = EventEnvelopeFactory.For(@event, AuthScope.SourceName, headers);
                    await _outbox.AppendAsync(envelope, cancellationToken);
                }
            }
        }
    }
}
