using Turboapi.Auth.Domain.Exceptions;

namespace Turboapi.Auth.Domain.Notifications
{
    /// <summary>
    /// A push-notification delivery target — one row per device/installation
    /// token (FCM/APNs). Keyed by the opaque provider token so re-registering
    /// the same device is an idempotent upsert. Not part of the Account
    /// aggregate: tokens churn independently of account state and are written
    /// on their own request path.
    /// </summary>
    public class DeviceToken
    {
        public string Token { get; private set; }
        public Guid AccountId { get; private set; }
        public string Platform { get; private set; }
        public DateTime CreatedAt { get; private set; }
        public DateTime LastSeenAt { get; private set; }

        public static readonly IReadOnlyCollection<string> SupportedPlatforms =
            new[] { "android", "ios", "web" };

        private DeviceToken() { }

        public DeviceToken(string token, Guid accountId, string platform)
        {
            if (string.IsNullOrWhiteSpace(token))
                throw new DomainException("Device token cannot be empty.");
            if (accountId == Guid.Empty)
                throw new DomainException("Device token must belong to an account.");

            Token = token;
            AccountId = accountId;
            Platform = NormalizePlatform(platform);
            CreatedAt = DateTime.UtcNow;
            LastSeenAt = CreatedAt;
        }

        /// <summary>Re-points an existing token row at the current account and refreshes liveness.</summary>
        public void Refresh(Guid accountId, string platform)
        {
            AccountId = accountId;
            Platform = NormalizePlatform(platform);
            LastSeenAt = DateTime.UtcNow;
        }

        private static string NormalizePlatform(string platform)
        {
            var normalized = (platform ?? string.Empty).Trim().ToLowerInvariant();
            return SupportedPlatforms.Contains(normalized) ? normalized : "unknown";
        }
    }
}
