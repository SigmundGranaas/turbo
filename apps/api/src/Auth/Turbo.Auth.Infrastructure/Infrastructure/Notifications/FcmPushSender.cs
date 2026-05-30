using System.Net.Http.Json;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using Turboapi.Auth.Application.Notifications;
using Turboapi.Auth.Domain.Interfaces;

namespace Turboapi.Auth.Infrastructure.Notifications
{
    /// <summary>
    /// Firebase Cloud Messaging implementation of <see cref="IPushSender"/>.
    ///
    /// SCAFFOLD: the fan-out (account -> device tokens), the FCM HTTP v1
    /// payload shape, and the per-token POST are all wired here. The one
    /// remaining step before notifications actually deliver is obtaining an
    /// OAuth2 access token from the Firebase service account — see
    /// <see cref="TryGetAccessTokenAsync"/>. Until credentials are configured
    /// (and that method is completed), <see cref="IsConfigured"/> is false and
    /// <see cref="SendAsync"/> is a logged no-op, so triggers can call it
    /// unconditionally today without breaking.
    /// </summary>
    public class FcmPushSender : IPushSender
    {
        private readonly IDeviceTokenRepository _deviceTokens;
        private readonly IHttpClientFactory _httpClientFactory;
        private readonly FcmOptions _options;
        private readonly ILogger<FcmPushSender> _logger;

        public FcmPushSender(
            IDeviceTokenRepository deviceTokens,
            IHttpClientFactory httpClientFactory,
            IOptions<FcmOptions> options,
            ILogger<FcmPushSender> logger)
        {
            _deviceTokens = deviceTokens;
            _httpClientFactory = httpClientFactory;
            _options = options.Value;
            _logger = logger;
        }

        public bool IsConfigured => _options.IsConfigured;

        public async Task SendAsync(PushNotification notification, CancellationToken cancellationToken = default)
        {
            var tokens = await _deviceTokens.GetByAccountAsync(notification.AccountId, cancellationToken);
            if (tokens.Count == 0)
            {
                _logger.LogDebug("No device tokens registered for account {AccountId}; skipping push.", notification.AccountId);
                return;
            }

            if (!IsConfigured)
            {
                _logger.LogDebug(
                    "FCM not configured (Notifications:Fcm); would have sent '{Title}' to {Count} device(s) for {AccountId}.",
                    notification.Title, tokens.Count, notification.AccountId);
                return;
            }

            var accessToken = await TryGetAccessTokenAsync(cancellationToken);
            if (accessToken == null)
            {
                _logger.LogWarning(
                    "FCM credentials are configured but the service-account access token could not be acquired; " +
                    "complete FcmPushSender.TryGetAccessTokenAsync to enable delivery. See docs/notifications/push-setup.md.");
                return;
            }

            var client = _httpClientFactory.CreateClient();
            var endpoint = $"https://fcm.googleapis.com/v1/projects/{_options.ProjectId}/messages:send";

            foreach (var device in tokens)
            {
                var payload = new
                {
                    message = new
                    {
                        token = device.Token,
                        notification = new { title = notification.Title, body = notification.Body },
                        data = notification.Data
                    }
                };

                using var request = new HttpRequestMessage(HttpMethod.Post, endpoint)
                {
                    Content = JsonContent.Create(payload)
                };
                request.Headers.Authorization = new System.Net.Http.Headers.AuthenticationHeaderValue("Bearer", accessToken);

                try
                {
                    var response = await client.SendAsync(request, cancellationToken);
                    if (response.StatusCode == System.Net.HttpStatusCode.NotFound ||
                        response.StatusCode == System.Net.HttpStatusCode.Gone)
                    {
                        // Stale token — FCM says the registration is no longer valid.
                        await _deviceTokens.RemoveAsync(device.Token, cancellationToken);
                    }
                    else if (!response.IsSuccessStatusCode)
                    {
                        _logger.LogWarning("FCM send to {Platform} device failed: {Status}", device.Platform, response.StatusCode);
                    }
                }
                catch (Exception ex) when (ex is not OperationCanceledException)
                {
                    _logger.LogWarning(ex, "FCM send to a {Platform} device threw.", device.Platform);
                }
            }
        }

        /// <summary>
        /// Acquires an OAuth2 bearer token for the FCM HTTP v1 API from the
        /// configured service account.
        ///
        /// TODO(push): implement using the service-account credentials in
        /// <see cref="FcmOptions"/> — e.g. add the Google.Apis.Auth package and
        /// return <c>await GoogleCredential.FromJson(json)
        ///   .CreateScoped("https://www.googleapis.com/auth/firebase.messaging")
        ///   .UnderlyingCredential.GetAccessTokenForRequestAsync()</c>.
        /// Returning null keeps delivery disabled without throwing.
        /// </summary>
        private Task<string?> TryGetAccessTokenAsync(CancellationToken cancellationToken)
            => Task.FromResult<string?>(null);
    }
}
