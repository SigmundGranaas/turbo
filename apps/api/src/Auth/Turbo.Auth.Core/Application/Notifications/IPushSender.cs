namespace Turboapi.Auth.Application.Notifications
{
    /// <summary>
    /// A single push notification addressed to a user. Triggers (friend
    /// requests, shares, …) construct one of these and hand it to
    /// <see cref="IPushSender"/>, which fans it out to the account's
    /// registered device tokens.
    /// </summary>
    public record PushNotification(
        Guid AccountId,
        string Title,
        string Body,
        IReadOnlyDictionary<string, string>? Data = null);

    /// <summary>
    /// Delivers push notifications to a user's devices. The concrete
    /// implementation (FCM) is a no-op until the deployment is configured
    /// with provider credentials, so callers can fire notifications
    /// unconditionally without guarding on configuration.
    /// </summary>
    public interface IPushSender
    {
        /// <summary>True when a real delivery backend is configured.</summary>
        bool IsConfigured { get; }

        /// <summary>Sends a notification to every device registered for the account.</summary>
        Task SendAsync(PushNotification notification, CancellationToken cancellationToken = default);
    }
}
