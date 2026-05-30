using Turboapi.Auth.Domain.Notifications;

namespace Turboapi.Auth.Domain.Interfaces
{
    public interface IDeviceTokenRepository
    {
        /// <summary>
        /// Registers (or refreshes) a device token for an account. Idempotent:
        /// re-registering an existing token updates its account/platform/liveness.
        /// </summary>
        Task RegisterAsync(Guid accountId, string token, string platform, CancellationToken cancellationToken = default);

        /// <summary>Removes a token (e.g. on logout / token rotation). No-op if absent.</summary>
        Task RemoveAsync(string token, CancellationToken cancellationToken = default);

        /// <summary>All active tokens for an account — the fan-out set when sending a push.</summary>
        Task<IReadOnlyList<DeviceToken>> GetByAccountAsync(Guid accountId, CancellationToken cancellationToken = default);
    }
}
