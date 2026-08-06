using Xunit;

public sealed class SessionStoreTests
{
    private static CreateSessionRequest Request(string? sessionId = null) => new(
        MeetingAgentContract.Schema,
        sessionId ?? Guid.NewGuid().ToString(),
        "meeting-1",
        MeetingAgentContract.Provider,
        "https://teams.microsoft.com/l/meetup-join/example",
        MeetingAgentContract.VisibleName,
        true);

    private static ProviderReceipt Receipt(string id, string state, bool recording = false) =>
        new(id, state, DateTimeOffset.UtcNow, recording);

    [Fact]
    public void RequestRequiresVisibleIdentityConsentAndTeamsUrl()
    {
        var invalid = Request() with
        {
            JoinUrl = "https://example.com/meeting",
            VisibleName = "Invisible bot",
            RequesterConfirmedVisibleDisclosure = false,
        };

        Assert.Equal(new[] { "visible_name", "visible_disclosure", "join_url" }, invalid.Validate());
    }

    [Fact]
    public void ProviderReceiptsDriveTheCompleteConsentedLifecycle()
    {
        var store = new SessionStore();
        var request = Request();
        store.Create(request);

        var session = store.ApplyReceipts(request.SessionId, new[]
        {
            Receipt("graph-1", MeetingSessionState.Invited),
            Receipt("graph-2", MeetingSessionState.Waiting),
            Receipt("graph-3", MeetingSessionState.Joined),
            Receipt("graph-4", MeetingSessionState.ConsentRequested),
            Receipt("graph-5", MeetingSessionState.ConsentGranted),
            Receipt("graph-6", MeetingSessionState.Transcribing, recording: true),
            Receipt("graph-7", MeetingSessionState.Paused, recording: true),
            Receipt("graph-8", MeetingSessionState.Leaving, recording: true),
            Receipt("graph-9", MeetingSessionState.Left, recording: true),
        });

        Assert.Equal(MeetingSessionState.Left, session.State);
        Assert.Equal(9, session.Events.Count);
        Assert.All(session.Events, item => Assert.Equal(MeetingAgentContract.VisibleName, item.Actor));
    }

    [Fact]
    public void DuplicateProviderReceiptIsIdempotent()
    {
        var store = new SessionStore();
        var request = Request();
        store.Create(request);
        var receipt = Receipt("graph-1", MeetingSessionState.Invited);

        store.ApplyReceipts(request.SessionId, new[] { receipt });
        var session = store.ApplyReceipts(request.SessionId, new[] { receipt });

        Assert.Single(session.Events);
    }

    [Fact]
    public void DuplicateSessionCannotEraseTheOriginalSession()
    {
        var store = new SessionStore();
        var request = Request();
        store.Create(request);

        Assert.Throws<InvalidOperationException>(() => store.Create(request));
        Assert.True(store.TryGet(request.SessionId, out var session));
        Assert.NotNull(session);
        Assert.Equal(MeetingSessionState.Planned, session!.State);
    }

    [Fact]
    public void TranscriptionRequiresConsentAndProviderRecordingConfirmation()
    {
        var store = new SessionStore();
        var request = Request();
        store.Create(request);
        store.ApplyReceipts(request.SessionId, new[]
        {
            Receipt("graph-1", MeetingSessionState.Invited),
            Receipt("graph-2", MeetingSessionState.Joined),
            Receipt("graph-3", MeetingSessionState.ConsentRequested),
            Receipt("graph-4", MeetingSessionState.ConsentGranted),
        });

        var error = Assert.Throws<InvalidOperationException>(() =>
            store.ApplyReceipts(request.SessionId, new[] { Receipt("graph-5", MeetingSessionState.Transcribing) }));

        Assert.Equal("recording-status-not-confirmed", error.Message);
    }

    [Fact]
    public void ImpossibleProviderTransitionDoesNotMutateSession()
    {
        var store = new SessionStore();
        var request = Request();
        store.Create(request);

        Assert.Throws<InvalidOperationException>(() =>
            store.ApplyReceipts(request.SessionId, new[] { Receipt("graph-1", MeetingSessionState.Transcribing, recording: true) }));
        Assert.True(store.TryGet(request.SessionId, out var session));
        Assert.NotNull(session);
        Assert.Equal(MeetingSessionState.Planned, session!.State);
        Assert.Empty(session.Events);
    }
}
