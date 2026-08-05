using System.Collections.Concurrent;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json.Serialization;

const int Schema = 1;
const string VisibleName = "Empathy AI — gravação e transcrição";

var builder = WebApplication.CreateBuilder(args);
builder.Services.AddSingleton<SessionStore>();
builder.Services.AddSingleton<TeamsRuntimeReadiness>();
var app = builder.Build();

app.Use(async (context, next) =>
{
    if (context.Request.Path == "/health") { await next(); return; }
    var configuredHash = Environment.GetEnvironmentVariable("EMPATHY_AGENT_PAIRING_TOKEN_SHA256")?.Trim().ToLowerInvariant();
    var authorization = context.Request.Headers.Authorization.ToString();
    var token = authorization.StartsWith("Bearer ", StringComparison.Ordinal) ? authorization[7..] : "";
    var suppliedHash = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(token))).ToLowerInvariant();
    if (string.IsNullOrWhiteSpace(configuredHash) || configuredHash.Length != 64 || !CryptographicOperations.FixedTimeEquals(Encoding.ASCII.GetBytes(configuredHash), Encoding.ASCII.GetBytes(suppliedHash)))
    {
        context.Response.StatusCode = StatusCodes.Status401Unauthorized;
        await context.Response.WriteAsJsonAsync(new { error = "unauthorized" });
        return;
    }
    await next();
});

app.MapGet("/health", () => Results.Ok(new { status = "ok", service = "empathy-teams-agent", schema = Schema }));

app.MapGet("/v1/readiness", (TeamsRuntimeReadiness readiness) =>
{
    var missing = readiness.MissingRequirements();
    return Results.Ok(new { ready = missing.Count == 0, missing, visible_name = VisibleName });
});

app.MapPost("/v1/sessions", (CreateSessionRequest request, TeamsRuntimeReadiness readiness, SessionStore store) =>
{
    var errors = request.Validate();
    if (errors.Count > 0) return Results.BadRequest(new { error = "invalid-request", details = errors });
    var missing = readiness.MissingRequirements();
    if (missing.Count > 0) return Results.Json(new { error = "service-not-ready", missing }, statusCode: StatusCodes.Status503ServiceUnavailable);

    // This branch must only be reachable once a reviewed TeamsCallRuntime is registered.
    // It intentionally cannot manufacture an "invited" event without a provider receipt.
    return Results.Json(new { error = "teams-runtime-not-registered" }, statusCode: StatusCodes.Status503ServiceUnavailable);
});

app.MapGet("/v1/sessions/{sessionId}", (string sessionId, SessionStore store) =>
    store.TryGet(sessionId, out var session) ? Results.Ok(session) : Results.NotFound(new { error = "session-not-found" }));

app.MapPost("/v1/sessions/{sessionId}/leave", (string sessionId, SessionStore store) =>
    store.TryGet(sessionId, out var session) ? Results.Ok(session) : Results.NotFound(new { error = "session-not-found" }));

app.Run();

sealed record CreateSessionRequest(
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
        if (Schema != 1) errors.Add("schema");
        if (!Guid.TryParse(SessionId, out _)) errors.Add("session_id");
        if (string.IsNullOrWhiteSpace(MeetingId) || MeetingId.Length > 200) errors.Add("meeting_id");
        if (Provider != "microsoft-teams") errors.Add("provider");
        if (VisibleName != "Empathy AI — gravação e transcrição") errors.Add("visible_name");
        if (!RequesterConfirmedVisibleDisclosure) errors.Add("visible_disclosure");
        if (!Uri.TryCreate(JoinUrl, UriKind.Absolute, out var uri) || uri.Scheme != Uri.UriSchemeHttps || !(uri.Host == "teams.microsoft.com" || uri.Host.EndsWith(".teams.microsoft.com", StringComparison.OrdinalIgnoreCase))) errors.Add("join_url");
        return errors;
    }
}

sealed class TeamsRuntimeReadiness
{
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
        // Removed only when the Microsoft Graph Calls/Media adapter is reviewed and registered.
        missing.Add("teams-graph-call-runtime");
        return missing;
    }

    private static void Require(string variable, string label, List<string> missing, string? expected = null)
    {
        var value = Environment.GetEnvironmentVariable(variable);
        if (string.IsNullOrWhiteSpace(value) || (expected is not null && !string.Equals(value, expected, StringComparison.OrdinalIgnoreCase))) missing.Add(label);
    }
}

sealed class SessionStore
{
    private readonly ConcurrentDictionary<string, object> sessions = new(StringComparer.Ordinal);
    public bool TryGet(string id, out object? value) => sessions.TryGetValue(id, out value);
}

public partial class Program { }
