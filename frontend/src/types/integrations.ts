export type IntegrationProvider = 'microsoft' | 'microsoft-teams' | 'zoom' | 'google-meet';
export type IntegrationCapabilityStage = 'local-ready' | 'provider-setup' | 'admin-consent' | 'external-review' | 'developer-preview';

export interface IntegrationFeatureFlags {
  schema: 1;
  outlook_calendar: boolean;
  outlook_mail_context: boolean;
  teams_agent: boolean;
  zoom_rtms: boolean;
  google_meet: boolean;
  google_meet_media_preview: boolean;
}

export interface IntegrationCapability {
  id: keyof Omit<IntegrationFeatureFlags, 'schema'>;
  provider: IntegrationProvider;
  name: string;
  description: string;
  stage: IntegrationCapabilityStage;
  prerequisites: string[];
  readsUserData: boolean;
  requiresExplicitAction: boolean;
}

export type ConnectorPermission =
  | 'calendar.basic'
  | 'mail.metadata'
  | 'mail.content'
  | 'meeting.participants'
  | 'meeting.artifacts'
  | 'meeting.realtime-media';

export interface ConnectedAccount {
  schema: 1;
  id: string;
  provider: IntegrationProvider;
  subject: string;
  tenant_id?: string | null;
  email: string;
  display_name: string;
  granted_permissions: ConnectorPermission[];
  token_expires_at?: string | null;
  connected_at: string;
  updated_at: string;
}

export interface MicrosoftAuthReadiness {
  configured: boolean;
  tenant: string;
  requested_scopes: string[];
  missing: string[];
}

export interface OutlookEventParticipant {
  display_name: string;
  email: string;
  response?: string | null;
}

export interface OutlookCalendarEvent {
  id: string;
  title: string;
  organizer?: OutlookEventParticipant | null;
  attendees: OutlookEventParticipant[];
  starts_at: string;
  ends_at: string;
  location?: string | null;
  join_url?: string | null;
  meeting_provider?: 'microsoft-teams' | 'zoom' | 'google-meet' | 'other' | null;
  web_url?: string | null;
}

export interface PreparedOutlookNote {
  note_id: string;
  folder_path: string;
  event: OutlookCalendarEvent;
}

export interface OutlookMailCandidate {
  id: string;
  subject: string;
  sender?: OutlookEventParticipant | null;
  to: OutlookEventParticipant[];
  cc: OutlookEventParticipant[];
  sent_at?: string | null;
  received_at?: string | null;
  conversation_id?: string | null;
  has_attachments: boolean;
  web_url?: string | null;
}

export interface OutlookSelectedMail extends OutlookMailCandidate {
  body_text: string;
  source_receipt: ContextSourceReceipt;
}

export interface ParticipantIdentity {
  id: string;
  display_name: string;
  email?: string | null;
  provider: IntegrationProvider | 'local';
  external_id?: string | null;
  source: 'calendar' | 'meeting' | 'note' | 'user';
  user_confirmed: boolean;
}

export interface ExternalMeeting {
  provider: Exclude<IntegrationProvider, 'microsoft'>;
  external_id: string;
  calendar_event_id?: string | null;
  title: string;
  organizer?: ParticipantIdentity | null;
  attendees: ParticipantIdentity[];
  starts_at: string;
  ends_at: string;
  join_url?: string | null;
}

export type MeetingAgentEventType =
  | 'invited'
  | 'waiting'
  | 'joined'
  | 'consent-requested'
  | 'consent-granted'
  | 'consent-denied'
  | 'transcribing'
  | 'paused'
  | 'left'
  | 'error';

export interface MeetingAgentEvent {
  schema: 1;
  event_id: string;
  meeting_id: string;
  provider: Exclude<IntegrationProvider, 'microsoft'>;
  event_type: MeetingAgentEventType;
  occurred_at: string;
  actor?: string | null;
  details?: string | null;
}

export interface ContextSourceReceipt {
  schema: 1;
  source_id: string;
  source_kind: 'calendar-event' | 'mail-message' | 'note' | 'transcript';
  provider: IntegrationProvider | 'local';
  title: string;
  occurred_at?: string | null;
  selected_by_user: boolean;
  content_included: boolean;
}
