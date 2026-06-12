namespace Turboapi.Places.Core;

/// <summary>
/// Serves the active dataset version (the ETag) from an in-process cache so the
/// hot path never scans the places table. The underlying store read is a 1-row
/// indexed lookup; the cache collapses it to ~0 cost per request and converges
/// within <see cref="_ttl"/> of a publish. TTL 0 (tests) always reads fresh.
/// </summary>
public sealed class DatasetVersionProvider
{
    private readonly IPlaceStore _store;
    private readonly TimeSpan _ttl;
    private readonly SemaphoreSlim _gate = new(1, 1);
    private string? _cached;
    private DateTime _fetchedUtc = DateTime.MinValue;

    public DatasetVersionProvider(IPlaceStore store, TimeSpan ttl)
    {
        _store = store;
        _ttl = ttl;
    }

    public async Task<string?> GetActiveVersionAsync(CancellationToken ct = default)
    {
        if (IsFresh()) return _cached;
        await _gate.WaitAsync(ct);
        try
        {
            if (IsFresh()) return _cached;
            _cached = await _store.GetActiveDatasetVersionAsync(ct);
            _fetchedUtc = DateTime.UtcNow;
            return _cached;
        }
        finally { _gate.Release(); }
    }

    private bool IsFresh() => _cached is not null && DateTime.UtcNow - _fetchedUtc < _ttl;
}
