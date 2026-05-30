namespace Turboapi.Auth.Infrastructure.Notifications
{
    /// <summary>
    /// Firebase Cloud Messaging configuration, bound from the
    /// "Notifications:Fcm" configuration section. Until both
    /// <see cref="ProjectId"/> and <see cref="ServiceAccountJson"/> (or
    /// <see cref="ServiceAccountJsonPath"/>) are supplied, the push sender
    /// stays in no-op mode.
    /// </summary>
    public class FcmOptions
    {
        public const string SectionName = "Notifications:Fcm";

        /// <summary>The Firebase project id (FCM HTTP v1 targets /v1/projects/{ProjectId}/messages:send).</summary>
        public string? ProjectId { get; set; }

        /// <summary>Inline service-account JSON (e.g. injected from a secret).</summary>
        public string? ServiceAccountJson { get; set; }

        /// <summary>Path to a service-account JSON file, as an alternative to <see cref="ServiceAccountJson"/>.</summary>
        public string? ServiceAccountJsonPath { get; set; }

        public bool IsConfigured =>
            !string.IsNullOrWhiteSpace(ProjectId) &&
            (!string.IsNullOrWhiteSpace(ServiceAccountJson) || !string.IsNullOrWhiteSpace(ServiceAccountJsonPath));
    }
}
