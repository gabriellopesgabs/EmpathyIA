using System.Security.Cryptography;
using System.Text;

var builder = WebApplication.CreateBuilder(args);
builder.Services.AddSingleton<SessionStore>();
builder.Services.AddSingleton<ITeamsCallRuntime, UnavailableTeamsCallRuntime>();
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

app.MapGet("/health", () => Results.Ok(new { status = "ok", service = "empathy-teams-agent", schema = MeetingAgentContract.Schema }));

app.MapGet("/v1/readiness", (TeamsRuntimeReadiness readiness) =>
{
    var missing = readiness.MissingRequirements();
    return Results.Ok(new { ready = missing.Count == 0, missing, visible_name = MeetingAgentContract.VisibleName });
});

app.MapPost("/v1/sessions", async (CreateSessionRequest request, TeamsRuntimeReadiness readiness, SessionStore store, ITeamsCallRuntime runtime, CancellationToken cancellationToken) =>
{
    var errors = request.Validate();
    if (errors.Count > 0) return Results.BadRequest(new { error = "invalid-request", details = errors });
    var missing = readiness.MissingRequirements();
    if (missing.Count > 0) return Results.Json(new { error = "service-not-ready", missing }, statusCode: StatusCodes.Status503ServiceUnavailable);
    var created = false;
    try
    {
        store.Create(request);
        created = true;
        var receipts = await runtime.JoinAsync(request, cancellationToken);
        if (receipts.Count == 0) throw new InvalidOperationException("provider-receipt-required");
        return Results.Created($"/v1/sessions/{request.SessionId}", store.ApplyReceipts(request.SessionId, receipts));
    }
    catch (Exception error)
    {
        if (created) store.Remove(request.SessionId);
        return Results.Json(new { error = "provider-join-failed", detail = error.Message }, statusCode: StatusCodes.Status502BadGateway);
    }
});

app.MapGet("/v1/sessions/{sessionId}", async (string sessionId, SessionStore store, ITeamsCallRuntime runtime, CancellationToken cancellationToken) =>
{
    if (!store.TryGet(sessionId, out var session)) return Results.NotFound(new { error = "session-not-found" });
    try
    {
        var receipts = await runtime.RefreshAsync(sessionId, cancellationToken);
        return Results.Ok(receipts.Count == 0 ? session : store.ApplyReceipts(sessionId, receipts));
    }
    catch (Exception error)
    {
        return Results.Json(new { error = "provider-refresh-failed", detail = error.Message }, statusCode: StatusCodes.Status502BadGateway);
    }
});

app.MapPost("/v1/sessions/{sessionId}/leave", async (string sessionId, SessionStore store, ITeamsCallRuntime runtime, CancellationToken cancellationToken) =>
{
    if (!store.TryGet(sessionId, out _)) return Results.NotFound(new { error = "session-not-found" });
    try
    {
        var receipts = await runtime.LeaveAsync(sessionId, cancellationToken);
        if (receipts.Count == 0) throw new InvalidOperationException("provider-receipt-required");
        return Results.Ok(store.ApplyReceipts(sessionId, receipts));
    }
    catch (Exception error)
    {
        return Results.Json(new { error = "provider-leave-failed", detail = error.Message }, statusCode: StatusCodes.Status502BadGateway);
    }
});

app.Run();

public partial class Program { }
