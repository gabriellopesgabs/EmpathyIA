#!/usr/bin/env bash
set -u

mode="${1:---report}"
if [[ "$mode" != "--report" && "$mode" != "--require-ready" ]]; then
  printf 'Uso: %s [--report|--require-ready]\n' "$0" >&2
  exit 64
fi

missing=()
ready=()

check_env() {
  local variable="$1"
  local label="$2"
  if [[ -n "${!variable:-}" ]]; then
    ready+=("$label")
  else
    missing+=("$label")
  fi
}

check_true() {
  local variable="$1"
  local label="$2"
  if [[ "${!variable:-}" == "true" ]]; then
    ready+=("$label")
  else
    missing+=("$label")
  fi
}

check_env EMPATHY_MICROSOFT_CLIENT_ID microsoft-client-id
check_env EMPATHY_E2E_TEST_ACCOUNT outlook-test-account
check_env EMPATHY_TEAMS_APP_ID teams-app-id
check_env EMPATHY_TEAMS_TENANT_ID teams-tenant-id
check_env EMPATHY_TEAMS_CERTIFICATE_THUMBPRINT teams-certificate
check_env EMPATHY_TEAMS_PUBLIC_BASE_URL teams-public-webhook
check_env EMPATHY_AGENT_PAIRING_TOKEN_SHA256 agent-pairing-token-hash
check_env EMPATHY_E2E_TEAMS_JOIN_URL teams-test-meeting
check_true EMPATHY_TEAMS_ADMIN_CONSENT_CONFIRMED tenant-admin-consent
check_true EMPATHY_TEAMS_RECORDING_POLICY_REVIEWED recording-policy-review

if [[ "${EMPATHY_TEAMS_PUBLIC_BASE_URL:-}" == https://* ]]; then
  ready+=("teams-public-webhook-https")
elif [[ -n "${EMPATHY_TEAMS_PUBLIC_BASE_URL:-}" ]]; then
  missing+=("teams-public-webhook-https")
fi

if [[ -f services/teams-agent/MicrosoftGraphCallsMediaRuntime.cs ]] \
  && ! grep -q 'UnavailableTeamsCallRuntime' services/teams-agent/Program.cs; then
  ready+=("teams-graph-call-runtime")
else
  missing+=("teams-graph-call-runtime")
fi

if [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* || "$(uname -s)" == CYGWIN* ]]; then
  ready+=("windows-runtime")
else
  missing+=("windows-runtime")
fi

printf 'Empathy meeting intelligence preflight\n'
printf '  ready:   %s\n' "${ready[*]:-none}"
printf '  missing: %s\n' "${missing[*]:-none}"
printf '  secrets: values were not printed\n'

if (( ${#missing[@]} > 0 )); then
  if [[ "$mode" == "--require-ready" ]]; then exit 2; fi
  exit 0
fi

printf '  result: ready for controlled Outlook + Teams homologation\n'
