using System.Collections.Concurrent;
using System.Text.Json.Serialization;

public static class MeetingAgentContract
{
    public const int Schema = 1;
    public const string Provider = "microsoft-teams";
    public const string VisibleName = "Empathy AI — gravação e transcrição";
}

public sealed record CreateSessionRequest(
    [property: JsonPropertyName("schema")] int Schema,
    [property: JsonPropertyName("session_id")] string SessionId,
    [property: JsonPropertyName("meeting_id")] string MeetingId,
    [property: JsonPropertyName("provider")] string Provider,
    [property: JsonPropertyName("join_url")] string JoinUrl,
    [property: JsonPropertyName("visible_name")] string VisibleName,
    [property: JsonPropertyName("requester_confirmed_visible_disclosure")] bool RequesterConfirmedVisibleDisclosure)
{
    public List<string> Validate()
    {
        var errors = new List<string>();
        if (Schema != MeetingAgentContract.Schema) errors.Add("schema");
        if (!Guid.TryParse(SessionId, out _)) errors.Add("session_id");
        if (string.IsNullOrWhiteSpace(MeetingId) || MeetingId.Length > 200) errors.Add("meeting_id");
        if (Provider != MeetingAgentContract.Provider) errors.Add("provider");
        if (VisibleName != MeetingAgentContract.VisibleName) errors.Add("visible_name");
        if (!RequesterConfirmedVisibleDisclosure) errors.Add("visible_disclosure");
        if (!Uri.TryCreate(JoinUrl, UriKind.Absolute, out var uri)
            || uri.Scheme != Uri.UriSchemeHttps
            || !(uri.Host == "teams.microsoft.com"
                || uri.Host.EndsWith(".teams.microsoft.com", StringComparison.OrdinalIgnoreCase)))
        {
            errors.Add("join_url");
        }
        return errors;
    }
}

public static class MeetingSessionState
{
    public const string Planned = "planned";
    public const string Invited = "invited";
    public const string Waiting = "waiting";
    public const string Joined = "joined";
    public const string ConsentRequested = "consent-requested";
    public const string ConsentGranted = "consent-granted";
    public const string ConsentDenied = "consent-denied";
    public const string Transcribing = "transcribing";
    public const string Paused = "paused";
    public const string Leaving = "leaving";
    public const string Left = "left";
    public const string Error = "error";
}

public sealed record ProviderReceipt(
    [property: JsonPropertyName("provider_event_id")] string ProviderEventId,
    [property: JsonPropertyName("state")] string State,
    [property: JsonPropertyName("occurred_at")] DateTimeOffset OccurredAt,
    [property: JsonPropertyName("recording_status_confirmed")] bool RecordingStatusConfirmed,
    [property: JsonPropertyName("details")] string? Details = null);

public sealed record MeetingAgentEvent(
    [property: JsonPropertyName("schema")] int Schema,
    [property: JsonPropertyName("event_id")] string EventId,
    [property: JsonPropertyName("session_id")] string SessionId,
    [property: JsonPropertyName("meeting_id")] string MeetingId,
    [property: JsonPropertyName("provider")] string Provider,
    [property: JsonPropertyName("state")] string State,
    [property: JsonPropertyName("occurred_at")] DateTimeOffset OccurredAt,
    [property: JsonPropertyName("actor")] string Actor,
    [property: JsonPropertyName("details")] string? Details,
    [property: JsonPropertyName("service_event_id")] string ServiceEventId,
    [property: JsonPropertyName("recording_status_confirmed")] bool RecordingStatusConfirmed);

public sealed record MeetingSession(
    [property: JsonPropertyName("schema")] int Schema,
    [property: JsonPropertyName("session_id")] string SessionId,
    [property: JsonPropertyName("meeting_id")] string MeetingId,
    [property: JsonPropertyName("provider")] string Provider,
    [property: JsonPropertyName("visible_name")] string VisibleName,
    [property: JsonPropertyName("state")] string State,
    [property: JsonPropertyName("events")] IReadOnlyList<MeetingAgentEvent> Events);

public interface ITeamsCallRuntime
{
    IReadOnlyList<string> MissingRequirements();
    Task<IReadOnlyList<ProviderReceipt>> JoinAsync(CreateSessionRequest request, CancellationToken cancellationToken);
    Task<IReadOnlyList<ProviderReceipt>> RefreshAsync(string sessionId, CancellationToken cancellationToken);
    Task<IReadOnlyList<ProviderReceipt>> LeaveAsync(string sessionId, CancellationToken cancellationToken);
}

public sealed class UnavailableTeamsCallRuntime : ITeamsCallRuntime
{
    private static InvalidOperationException NotRegistered() =>
        new("The reviewed Microsoft Graph Calls/Media runtime is not registered.");

    public IReadOnlyList<string> MissingRequirements() => new[] { "teams-graph-call-runtime" };
    public Task<IReadOnlyList<ProviderReceipt>> JoinAsync(CreateSessionRequest request, CancellationToken cancellationToken) => throw NotRegistered();
    public Task<IReadOnlyList<ProviderReceipt>> RefreshAsync(string sessionId, CancellationToken cancellationToken) => throw NotRegistered();
    public Task<IReadOnlyList<ProviderReceipt>> LeaveAsync(string sessionId, CancellationToken cancellationToken) => throw NotRegistered();
}

public sealed class SessionStore
{
    private readonly ConcurrentDictionary<string, MeetingSession> sessions = new(StringComparer.Ordinal);

    public bool TryGet(string id, out MeetingSession? value) => sessions.TryGetValue(id, out value);

    public MeetingSession Create(CreateSessionRequest request)
    {
        var session = new MeetingSession(
            MeetingAgentContract.Schema,
            request.SessionId,
            request.MeetingId,
            MeetingAgentContract.Provider,
            MeetingAgentContract.VisibleName,
            MeetingSessionState.Planned,
            Array.Empty<MeetingAgentEvent>());
        if (!sessions.TryAdd(request.SessionId, session))
        {
            throw new InvalidOperationException("session-already-exists");
        }
        return session;
    }

    public MeetingSession ApplyReceipts(string sessionId, IReadOnlyList<ProviderReceipt> receipts)
    {
        if (!sessions.TryGetValue(sessionId, out var current))
        {
            throw new KeyNotFoundException("session-not-found");
        }

        var events = current.Events.ToList();
        foreach (var receipt in receipts)
        {
            if (string.IsNullOrWhiteSpace(receipt.ProviderEventId)
                || receipt.ProviderEventId.Length > 200
                || receipt.Details?.Length > 4_000)
            {
                throw new InvalidOperationException("provider-receipt-invalid");
            }
            if (events.Any(item => item.ServiceEventId == receipt.ProviderEventId)) continue;
            ValidateTransition(current.State, receipt);
            var serviceEventId = receipt.ProviderEventId;
            events.Add(new MeetingAgentEvent(
                MeetingAgentContract.Schema,
                Guid.NewGuid().ToString(),
                current.SessionId,
                current.MeetingId,
                MeetingAgentContract.Provider,
                receipt.State,
                receipt.OccurredAt,
                MeetingAgentContract.VisibleName,
                receipt.Details,
                serviceEventId,
                receipt.RecordingStatusConfirmed));
            current = current with { State = receipt.State, Events = events.ToArray() };
        }
        sessions[sessionId] = current;
        return current;
    }

    public void Remove(string sessionId) => sessions.TryRemove(sessionId, out _);

    private static void ValidateTransition(string current, ProviderReceipt receipt)
    {
        if (receipt.State == MeetingSessionState.Transcribing && !receipt.RecordingStatusConfirmed)
        {
            throw new InvalidOperationException("recording-status-not-confirmed");
        }
        var allowed = current switch
        {
            MeetingSessionState.Planned => receipt.State is MeetingSessionState.Invited or MeetingSessionState.Left or MeetingSessionState.Error,
            MeetingSessionState.Invited => receipt.State is MeetingSessionState.Waiting or MeetingSessionState.Joined or MeetingSessionState.Leaving or MeetingSessionState.Left or MeetingSessionState.Error,
            MeetingSessionState.Waiting => receipt.State is MeetingSessionState.Joined or MeetingSessionState.Leaving or MeetingSessionState.Left or MeetingSessionState.Error,
            MeetingSessionState.Joined => receipt.State is MeetingSessionState.ConsentRequested or MeetingSessionState.Leaving or MeetingSessionState.Left or MeetingSessionState.Error,
            MeetingSessionState.ConsentRequested => receipt.State is MeetingSessionState.ConsentGranted or MeetingSessionState.ConsentDenied or MeetingSessionState.Leaving or MeetingSessionState.Left or MeetingSessionState.Error,
            MeetingSessionState.ConsentGranted => receipt.State is MeetingSessionState.Transcribing or MeetingSessionState.Paused or MeetingSessionState.Leaving or MeetingSessionState.Left or MeetingSessionState.Error,
            MeetingSessionState.ConsentDenied => receipt.State is MeetingSessionState.Leaving or MeetingSessionState.Left or MeetingSessionState.Error,
            MeetingSessionState.Transcribing => receipt.State is MeetingSessionState.Paused or MeetingSessionState.Leaving or MeetingSessionState.Error,
            MeetingSessionState.Paused => receipt.State is MeetingSessionState.Transcribing or MeetingSessionState.Leaving or MeetingSessionState.Error,
            MeetingSessionState.Leaving => receipt.State is MeetingSessionState.Left or MeetingSessionState.Error,
            MeetingSessionState.Left => false,
            MeetingSessionState.Error => receipt.State is MeetingSessionState.Leaving or MeetingSessionState.Left,
            _ => false,
        };
        if (!allowed) throw new InvalidOperationException($"invalid-transition:{current}->{receipt.State}");
    }
}

public sealed class TeamsRuntimeReadiness
{
    private readonly ITeamsCallRuntime runtime;

    public TeamsRuntimeReadiness(ITeamsCallRuntime runtime) => this.runtime = runtime;

    public List<string> MissingRequirements()
    {
        var missing = new List<string>();
        Require("EMPATHY_TEAMS_APP_ID", "teams-app-id", missing);
        Require("EMPATHY_TEAMS_TENANT_ID", "teams-tenant-id", missing);
        Require("EMPATHY_TEAMS_CERTIFICATE_THUMBPRINT", "certificate", missing);
        Require("EMPATHY_TEAMS_PUBLIC_BASE_URL", "public-webhook", missing);
        Require("EMPATHY_TEAMS_ADMIN_CONSENT_CONFIRMED", "tenant-admin-consent", missing, "true");
        Require("EMPATHY_TEAMS_RECORDING_POLICY_REVIEWED", "recording-policy-review", missing, "true");
        if (!OperatingSystem.IsWindows()) missing.Add("windows-server-runtime");
        missing.AddRange(runtime.MissingRequirements());
        return missing.Distinct(StringComparer.Ordinal).ToList();
    }

    private static void Require(string variable, string label, List<string> missing, string? expected = null)
    {
        var value = Environment.GetEnvironmentVariable(variable);
        if (string.IsNullOrWhiteSpace(value)
            || (expected is not null && !string.Equals(value, expected, StringComparison.OrdinalIgnoreCase)))
        {
            missing.Add(label);
        }
    }
}
