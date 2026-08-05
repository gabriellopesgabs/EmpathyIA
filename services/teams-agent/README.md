# Empathy Teams Agent service

This is the hardened service boundary for the visible participant named
**Empathy AI — gravação e transcrição**. It is intentionally fail closed.

The HTTP contract, pairing-token authentication, request validation and exact
readiness diagnostics are implemented. Session creation remains unavailable
while `teams-graph-call-runtime` appears in readiness. Removing that gate
requires a reviewed implementation using Microsoft's Calls/Meetings SDK and
application-hosted media runtime; status events must come from provider
receipts, never timers or optimistic UI.

Production prerequisites include a Windows Server workload in Azure, a public
calling webhook, certificate-based app identity, tenant admin consent, the
current `Microsoft.Graph.Communications.Calls.Media` package, and a reviewed
recording policy. `updateRecordingStatus` success must precede any
`transcribing` event or persistence of media-derived data.

The pairing token is supplied as a SHA-256 hex digest in
`EMPATHY_AGENT_PAIRING_TOKEN_SHA256`. Never place the raw token in an image,
repository, environment example or Markdown note.
