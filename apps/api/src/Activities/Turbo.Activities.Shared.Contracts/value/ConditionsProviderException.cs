namespace Turboapi.Activities.value;

/// <summary>
/// Thrown by a conditions provider when an upstream call fails in a way
/// that callers should surface as "conditions unavailable" rather than
/// propagating as a generic InvalidOperationException. Wrap raw HTTP /
/// parsing failures with this so application code can distinguish
/// "upstream is down" from "we asked for something invalid".
/// </summary>
public sealed class ConditionsProviderException : Exception
{
    public ConditionsProviderException(string message) : base(message) { }
    public ConditionsProviderException(string message, Exception inner) : base(message, inner) { }
}
