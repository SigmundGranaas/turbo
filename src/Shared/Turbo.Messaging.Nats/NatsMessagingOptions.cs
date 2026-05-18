namespace Turbo.Messaging.Nats;

/// <summary>
/// Configuration for the NATS JetStream publish + subscribe path.
/// Per-module values keep stream and subject namespaces separate so an
/// audit-style consumer for one module never sees events from another
/// module by accident.
/// </summary>
public sealed class NatsMessagingOptions
{
    /// <summary>NATS server URL, e.g. <c>nats://localhost:4222</c>.</summary>
    public string Url { get; set; } = "nats://localhost:4222";

    /// <summary>JetStream stream name (auto-created if missing).</summary>
    public string StreamName { get; set; } = "TURBO";

    /// <summary>Subjects this stream binds, e.g. <c>turbo.activity.&gt;</c>.</summary>
    public string[] Subjects { get; set; } = ["turbo.>"];

    /// <summary>Stream retention age. Default 14 days.</summary>
    public TimeSpan MaxAge { get; set; } = TimeSpan.FromDays(14);

    /// <summary>Number of replicas (1 in dev, 3 in prod).</summary>
    public int Replicas { get; set; } = 1;

    /// <summary>Subject prefix this module owns, e.g. <c>turbo.activity</c>. Used by the subscriber host to bind durable consumers only for its own subjects.</summary>
    public string SubjectPrefix { get; set; } = "turbo";
}
