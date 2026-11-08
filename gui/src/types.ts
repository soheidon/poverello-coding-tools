// ---- Health ----
export interface HealthStatus {
  status: string;
  upstream: string;
}

// ---- Port process ----
export interface PortProcess {
  pid: string;
  raw_output: string;
}

// ---- Config ----
// ---- OpenRouter multi-profile ----
// ---- Claude Code auto-compact ----
export interface ClaudeCodeAutoCompactConfig {
  enabled: boolean;
  trigger_percent: number;
}

export type ClaudeCodeTargetMode = "auto" | "manual" | "claude_default";

export interface ClaudeCodeTargetConfig {
  mode: ClaudeCodeTargetMode;
  window_tokens?: number;
  trigger_percent?: number;
}

export type ClaudeCodeActiveRoute = "gateway" | "ollama";

export interface ClaudeCodeThirdPartyConfig {
  show_on_dashboard: boolean;
  enabled?: boolean;
  provider: "ollama";
  base_url?: string;
  model?: string;
  models?: Record<string, string>;
  thinking_mode?: "normal" | "thinking";
  supports_vision?: boolean;
  context_window?: number | null;
}

export interface ClaudeCodeRootSection {
  auto_compact: ClaudeCodeAutoCompactConfig;
  active_route?: ClaudeCodeActiveRoute;
  third_party_provider?: ClaudeCodeThirdPartyConfig | null;
}

export interface ClaudeCodeProviderSection {
  auto_compact: ClaudeCodeTargetConfig;
}

export type AutoCompactMode = "auto" | "manual" | "claude_default";
export type AutoCompactStatus = "applied" | "disabled" | "incomplete";
export type ContextWindowSource =
  | "official"
  | "provider_api"
  | "builtin"
  | "user"
  | "unknown";

export interface EffectiveContextRoute {
  route: string;
  upstreamModel: string | null;
  contextWindowTokens: number | null;
  contextWindowSource: ContextWindowSource;
}

export interface EffectiveAutoCompact {
  globallyEnabled: boolean;
  mode: AutoCompactMode;
  status: AutoCompactStatus;
  applyEnvironment: boolean;
  windowTokens: number | null;
  triggerPercent: number | null;
  estimatedTriggerTokens: number | null;
  targetKind: "provider" | "profile" | "claude_code_3p" | null;
  targetId: string | null;
  targetName: string | null;
  routes: EffectiveContextRoute[];
}

export interface ClaudeCodeLaunchCommand {
  command: string;
  applyEnvironment: boolean;
  status: AutoCompactStatus;
}

export interface OpenRouterProfile {
  id: string;
  display_name: string;
  model_map: Record<string, string>;
  visible_models: string[];
  models: Record<string, ModelEntry>;
  hidden?: boolean;
  claude_code?: ClaudeCodeProviderSection;
}

export interface GatewayProfileRoute {
  provider: string;
  model?: string | null;
  /** Matches the serialized Rust GatewayProfileRoute field. */
  profile_id?: string | null;
  thinking_mode?: string | null;
  reasoning_effort?: string | null;
}

export interface GatewayProfile {
  id: string;
  display_name: string;
  routes: Record<string, GatewayProfileRoute>;
  hidden?: boolean;
  claude_code?: ClaudeCodeProviderSection;
}

export interface CommandOutcome {
  restart_gateway: boolean;
  restart_reason: string;
}

export interface CommandResponse<T = unknown> {
  value: T;
  restartGateway: boolean;
  restartReason: string;
}

export interface ModelEntry {
  upstream_model: string;
  thinking_mode?: string; // "normal" | "thinking" | "thinking_only" — user choice
  // Deprecated capability fields — read for backward compat, not written on save.
  // Capabilities are resolved from MODEL_CAPABILITIES (app code) by upstream_model name.
  thinking?: string;
  supports_vision?: boolean;
  supports_video?: boolean;
  visible?: boolean;
  force_thinking?: boolean;
  supports_non_thinking?: boolean;
  reasoning_effort?: string;
  supports_image_url?: boolean;
  supports_image_base64?: boolean;
  supports_video_url?: boolean;
  supports_video_base64?: boolean;
}

export interface ProviderConfig {
  display_name: string;
  upstream_url: string;
  api_key_env: string;
  default_model: string;
  force_anthropic_version: string | null;
  supports_count_tokens: boolean;
  supports_vision: boolean;
  supports_video: boolean;
  supports_thinking: boolean;
  model_map: Record<string, string>;
  visible_models: string[];
  models?: Record<string, ModelEntry>;
  /** Legacy GLM contract/protocol block from older configs: preserved on disk, never read or written by the UI. */
  zai?: unknown;
  profiles?: OpenRouterProfile[];
  claude_code?: ClaudeCodeProviderSection;
  hidden?: boolean;
}

export interface ServerConfig {
  host: string;
  port: number;
  enable_cors: boolean;
}

export interface McpTargetConfig {
  model?: string;
  thinking_mode?: string;
  reasoning_effort?: string;
}

export function getMcpTargetKey(providerId: string, profileId?: string | null): string {
  if (providerId === "openrouter" && profileId) {
    return `openrouter:${profileId}`;
  }
  return providerId;
}

export interface McpConfig {
  provider?: string;
  profile_id?: string;
  model?: string;
  thinking_mode?: string;
  reasoning_effort?: string;
  targets?: Record<string, McpTargetConfig>;
}

export interface McpStatus {
  configured: boolean;
  provider?: string;
  profile_name?: string;
  model?: string;
  thinking_mode?: string;
  reasoning_effort?: string;
  api_key_set: boolean;
  api_key_env?: string;
}

export type AntigravityMcpStatus = "not_configured" | "configured" | "invalid";

export interface AntigravityMcpInfo {
  status: AntigravityMcpStatus;
  config_path: string;
  config_dir: string;
  registered_command?: string | null;
  registered_args?: string[] | null;
  error?: string | null;
  legacy_status?: AntigravityMcpStatus | null;
  legacy_command?: string | null;
  legacy_args?: string[] | null;
}

export type AntigravitySkillStatus = "not_installed" | "installed" | "outdated" | "invalid";

export interface AntigravityCommandItemInfo {
  name: string;
  slash_command: string;
  status: AntigravitySkillStatus;
  skill_path: string;
  error?: string | null;
}

export interface AntigravityCommandsInfo {
  skills_dir: string;
  commands?: AntigravityCommandItemInfo[];
  plan_command: AntigravityCommandItemInfo;
  revise_command: AntigravityCommandItemInfo;
  review_command: AntigravityCommandItemInfo;
  legacy_plan_command?: AntigravityCommandItemInfo | null;
  legacy_revise_command?: AntigravityCommandItemInfo | null;
  legacy_review_command?: AntigravityCommandItemInfo | null;
  install_all_results?: AntigravityCommandInstallResult[] | null;
}

export type AntigravityGlobalRuleId = "deepseek_harness" | "codex_cli";

export type GlobalRuleTemplateStatus = "not_installed" | "installed" | "modified_or_protected";

export interface GlobalRuleItemInfo {
  id: string;
  file_name: string;
  path: string;
  status: GlobalRuleTemplateStatus;
  can_remove: boolean;
  error?: string | null;
}

export type ReviewerPolicy = "none" | "antigravity" | "codex_only" | "deepseek_only" | "codex_plus_deepseek";
export type ReviewerPreset = "fast" | "standard" | "strict" | "custom";
export type AntigravityPriorityReviewer = "codex_cli" | "deepseek";

export interface CodexReviewerConfig {
  model: string;
  reasoning_effort: string;
}

export interface DeepSeekReviewerConfig {
  model: string;
  reasoning_effort: string;
}

export interface ReviewerPolicyConfig {
  schema_version: number;
  policy: ReviewerPolicy;
  plan_review_policy?: ReviewerPolicy | null;
  plan_priority_reviewer?: AntigravityPriorityReviewer | null;
  plan_reviewers?: string[] | null;
  build_review_policy?: ReviewerPolicy | null;
  build_priority_reviewer?: AntigravityPriorityReviewer | null;
  build_reviewers?: string[] | null;
  preset?: ReviewerPreset | null;
  codex?: CodexReviewerConfig | null;
  deepseek?: DeepSeekReviewerConfig | null;
  priority_reviewer?: AntigravityPriorityReviewer | null;
  unconfirmed_migration?: boolean;
  content_hash?: string | null;
  active_plan_path?: string | null;
  planner_role?: string | null;
  reviser_role?: string | null;
}

export interface WorkflowSkillsPolicyStatus {
  compliant: boolean;
  details: AntigravityCommandItemInfo[];
}

export type CliToolStatusType = "not_installed" | "installed" | "execution_error" | "unknown";

export interface CliToolInfo {
  name: string;
  display_name: string;
  installed: boolean;
  path: string | null;
  installed_version: string | null;
  latest_version: string | null;
  latest_checked_at: string | null;
  package_name: string;
  is_preview: boolean;
  update_available: boolean;
  check_id: string | null;
  prerequisite_error: string | null;
  execution_error?: string | null;
  latest_check_error?: string | null;
  outcome_unknown?: string | null;
  status?: CliToolStatusType;
}

export interface NodePrerequisiteStatus {
  node_installed: boolean;
  node_version: string | null;
  node_major_version: number | null;
  node_path: string | null;
  npm_installed: boolean;
  npm_version: string | null;
  npm_path: string | null;
  node_npm_ready: boolean;
  gemini_node_ready: boolean;
  message: string | null;
  npm_global_prefix?: string | null;
}

export type CliAuthStatusType = "authenticated" | "not_authenticated" | "unknown";

export interface CliAuthInfo {
  name: string;
  status: CliAuthStatusType;
  auth_type: string | null;
  message: string | null;
  can_launch_login: boolean;
  account_identity?: string | null;
  codex_home?: string | null;
  last_checked_at?: string | null;
}

export function mapReviewerToCliId(reviewerId: string): string {
  if (reviewerId === "deepseek") {
    return "dsh";
  }
  return reviewerId;
}

export type CodexSkillState =
  | "NotInstalled"
  | "Installed"
  | "UpdateAvailable"
  | "ProtectedConflict"
  | "RecoveryRequired"
  | "Blocked";

export interface CodexSkillItemInfo {
  name: string;
  display_name: string;
  description: string;
  state: CodexSkillState;
  path: string;
  discovery_verified: boolean;
  detail_message: string | null;
  has_backup: boolean;
}

export interface CodexSkillsStatus {
  skills_dir: string;
  skills: CodexSkillItemInfo[];
  resolution_error: string | null;
}

export type CodexGlobalRulesState =
  | "NotInstalled"
  | "Installed"
  | "UpdateAvailable"
  | "Shadowed"
  | "Conflict"
  | "Blocked";

export interface CodexGlobalRulesStatus {
  codex_home: string;
  agents_md_path: string;
  state: CodexGlobalRulesState;
  is_shadowed: boolean;
  shadowed_by: string | null;
  detail_message: string | null;
  resolution_error: string | null;
}

export type ReviewMode =
  | "codex_self_only"
  | "external_only"
  | "codex_plus_external"
  | "none";

export interface ReviewersSelection {
  codex: boolean;
  gemini: boolean;
  deepseek: boolean;
}

export interface ExternalReviewerConfig {
  model?: string;
  reasoning_effort?: string;
}

export interface CodexReviewSettings {
  schema_version: number;
  review_mode: ReviewMode;
  reviewers: ReviewersSelection;
  priority_reviewer: string | null;
  gemini_cli_enabled: boolean;
  unconfirmed_migration?: boolean;
  codex?: ExternalReviewerConfig;
  deepseek?: ExternalReviewerConfig;
  gemini?: ExternalReviewerConfig;
  active_plan_path?: string | null;
  planner_role?: string | null;
  reviser_role?: string | null;
  plan_reviewers?: string[] | null;
  build_reviewers?: string[] | null;
}

export interface PlanFileEntry {
  path: string;
  relative_path: string;
  title?: string | null;
  status?: string | null;
  last_modified_ms: number;
  is_active: boolean;
}

export interface PlanFileDetails {
  repo_root: string;
  plans_dir: string;
  selected_folder?: string | null;
  active_plan_path?: string | null;
  active_plan_relative_path?: string | null;
  plans: PlanFileEntry[];
}

export interface ActivePlanFileDetails {
  rel_path: string;
  abs_path: string;
  file_name: string;
  title: string;
  status?: string | null;
  content: string;
  sha256_hash: string;
  file_identity?: string | null;
  last_modified_unix: number;
}

export interface PlanCreateRequest {
  name: string;
  folder?: string | null;
  title?: string | null;
  set_as_active?: boolean;
}

export interface AuditTrailEvent {
  phase: string;
  role_actor: string;
  result: string;
  notes: string;
  expected_sha256: string;
}

export interface CodexAccountInfo {
  installed: boolean;
  executable_path: string | null;
  version: string | null;
  auth_method: string | null;
  account_identity: string | null;
  config_profile: string | null;
  requires_openai_auth: boolean | null;
  codex_home?: string | null;
  last_checked_at?: string | null;
}

export interface AntigravityGlobalRuleStatus {
  id: AntigravityGlobalRuleId;
  file_name: string;
  path: string;
  exists: boolean;
  content: string;
  content_hash: string;
}

export type AntigravityCommandInstallOutcome = "installed" | "already_current" | "refused" | "failed" | "not_attempted";

export interface AntigravityCommandInstallResult {
  name: string;
  outcome: AntigravityCommandInstallOutcome;
  message?: string | null;
}

export type WebResearchPolicy = "requested_only" | "requested_or_necessary" | "unrestricted";
export type ResponseVerbosityPolicy = "minimal" | "concise" | "standard" | "detailed";
export type ExplorationPolicy = "minimal" | "standard" | "detailed";
export type VerificationPolicy = "focused" | "standard" | "broad";

export interface PovDirectSettings {
  auto_activation: boolean;
  web_research: WebResearchPolicy;
  response_verbosity: ResponseVerbosityPolicy;
  repository_exploration: ExplorationPolicy;
  verification: VerificationPolicy;
}

export type PovDirectFileState =
  | "absent"
  | "managed_synced"
  | "managed_drifted"
  | "legacy_poverello"
  | "unmanaged_custom";

export type PovDirectApplyOperation =
  | "apply"
  | "migrate_with_backup"
  | "adopt_with_backup";

export interface PovDirectApplyRequest {
  settings: PovDirectSettings;
  operation: PovDirectApplyOperation;
  expected_state: PovDirectFileState;
  expected_content_hash: string;
}

export interface PovDirectStatus {
  rule_path: string;
  skill_path: string;
  skill_present: boolean;
  state: PovDirectFileState;
  settings_state: "embedded_valid" | "defaults_pending" | "legacy_requires_import" | "metadata_invalid";
  settings: PovDirectSettings;
  preview_text: string;
  expanded_byte_size: number;
  byte_limit: number;
  can_apply: boolean;
  available_operations: PovDirectApplyOperation[];
  block_reason?: string | null;
  content_hash: string;
  legacy_sidecar_present: boolean;
}

// ---- Poverello Nerf (/pov-nerf-balanced & /pov-nerf-lightweight) ----
export type PovNerfSkillId = "balanced" | "lightweight";
export type PovNerfSkillState = "not_installed" | "installed" | "outdated" | "modified" | "invalid";
export type PovNerfAction = "install" | "update" | "remove";

export interface PovNerfSkillEntry {
  skill_id: PovNerfSkillId;
  skill_name: string;
  slash_command: string;
  target_path: string;
  state: PovNerfSkillState;
  permitted_actions: PovNerfAction[];
  diagnostic_reason: string | null;
}

export interface LegacySingleSkillNotice {
  present: boolean;
  path: string;
  message: string;
}

export interface PovNerfCommandsInfo {
  skills_dir: string;
  skills: PovNerfSkillEntry[];
  legacy_single_skill: LegacySingleSkillNotice | null;
}

export type PovNerfMutationOutcome = "success" | "refused" | "verification_unavailable";

export interface PovNerfOperationResult {
  outcome: PovNerfMutationOutcome;
  skill: PovNerfSkillEntry;
  message: string | null;
}

import type { OrchestratorConfig } from "./types/orchestrator";

export interface GatewayConfig {
  config_version?: string;
  active_provider: string | null;
  active_openrouter_profile_id?: string | null;
  active_gateway_profile_id?: string | null;
  gateway_profiles?: GatewayProfile[];
  providers: Record<string, ProviderConfig>;
  server: ServerConfig;
  non_vision_image_policy?: string;
  normalize_response_model_identity?: boolean;
  claude_code?: ClaudeCodeRootSection;
  mcp?: McpConfig;
  orchestrator?: OrchestratorConfig;
}

// ---- API Key ----
export interface ApiKeyStatus {
  set: boolean;
  env_var: string;
}

export type AllApiKeyStatus = Record<string, ApiKeyStatus>;

// ---- Log ----
export interface LogContent {
  filename: string;
  content: string;
  line_count: number;
}

// ---- Log list entry ----
export interface LogListEntry {
  filename: string;
  size: number;
}

// ---- Raw config (for editing with encoding) ----
export interface RawConfigResponse {
  content: string;
  encoding_used: string;
  config_path: string;
}

// ---- Write config response ----
export interface WriteConfigResponse {
  saved_encoding: string;
}

// ---- Claude config discovery ----
export interface ClaudeConfigCandidate {
  path: string;
  exists: boolean;
  likely_config: boolean;
}

// ---- Proxy start result ----
export interface StartProxyResult {
  success: boolean;
  pid: number;
  python: string;
  dir: string;
  log: string;
}

// ---- Gateway status (used by dashboard) ----
export interface GatewayStatus {
  reachable: boolean;
  port_listening: boolean;
  checked_at: string;
  error: string | null;
  managed_child_running: boolean;
  managed_child_pid: number | null;
  diagnostic: string;
}

// ---- Hook shape ----
export interface AsyncState<T> {
  data: T | null;
  error: string | null;
  loading: boolean;
  refresh: () => void;
}

// ---- Reviewer Diagnostics (§18) ----
export interface ReviewerDiagnosticEvent {
  run_id: string;
  timestamp: string;
  reviewer: string;
  model: string | null;
  reasoning_effort: string | null;
  snapshot_files_count: number;
  snapshot_total_bytes: number;
  snapshot_sha256: string | null;
  sandbox_mechanism: string;
  sandbox_verified: boolean;
  execution_phase: string;
  process_spawned: boolean;
  exit_code: number | null;
  timed_out: boolean;
  cancelled: boolean;
  children_reaped: boolean;
  verdict: string | null;
  error_category: string | null;
  external_dispatch_occurred: boolean;
  duration_ms: number;
}

export interface ExecutionCheckpoint {
  checkpoint_version: number;
  workflow: string;
  status: string;
  last_completed: string;
  next_action: string;
  repository: string;
  active_plan: string;
  plan_revision: string;
  workflow_run_id: string;
  completed_work: string[];
  modified_files: string[];
  validation: string[];
  reviewer: string;
  review_result: string;
  unresolved_findings: string[];
  pending_work: string[];
  timestamp: string;
}

export interface ResumptionPreflightResult {
  is_resumable: boolean;
  next_action: string;
  active_plan: string;
  workflow: string;
  status: string;
  reasons: string[];
}
