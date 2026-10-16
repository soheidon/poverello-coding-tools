use super::*;
use crate::orchestrator::plan_workspace::{
    capture_frozen_plan_snapshot, plan_append_with_context_validation, plan_current,
    resolve_plan_context, PlanAppendRequest, PlanWorkspaceConfig,
};
use crate::orchestrator::types::{
    AgentRole, ExecutionAdapterType, OrchestratorProfile, ProfileCapability,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

// ===========================================================================
// Test Helpers
// ===========================================================================

const DUMMY_HEX_64: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const DUMMY_UUID: &str = "11111111-1111-1111-1111-111111111111";

fn setup_test_workspace() -> (TempDir, PathBuf, PathBuf) {
    let temp = TempDir::new().expect("create temp dir");
    let root = temp.path().join("project");
    fs::create_dir_all(root.join("gui/src-tauri")).expect("create src-tauri dir");
    let plan_dir = root.join(".plan");
    fs::create_dir_all(&plan_dir).expect("create .plan dir");

    let runs_dir = root.join(".runs");
    fs::create_dir_all(&runs_dir).expect("create .runs dir");

    fs::write(
        root.join("gui/package.json"),
        br#"{"name": "test", "version": "0.23.0"}"#,
    )
    .expect("write package.json");

    fs::write(
        root.join("gui/src-tauri/tauri.conf.json"),
        br#"{"productName": "test", "version": "0.23.0"}"#,
    )
    .expect("write tauri.conf.json");

    fs::write(
        root.join("gui/src-tauri/Cargo.toml"),
        br#"[package]
name = "test"
version = "0.23.0"
"#,
    )
    .expect("write Cargo.toml");

    let initial_content = "# Test Plan\n\n## Overview\nInitial plan content.\n";
    fs::write(plan_dir.join("V0.23.0-r1.md"), initial_content).expect("write plan");

    (temp, root, runs_dir)
}

fn sample_profile(id: &str, display_name: &str, model: &str) -> OrchestratorProfile {
    OrchestratorProfile {
        id: id.to_string(),
        display_name: display_name.to_string(),
        adapter: ExecutionAdapterType::Provider,
        capabilities: vec![ProfileCapability::Reasoning, ProfileCapability::Review],
        provider_id: Some("deepseek".to_string()),
        provider_profile_id: None,
        model: Some(model.to_string()),
        thinking_mode: None,
        reasoning_effort: None,
        ollama_model: None,
        ollama_endpoint: None,
        executable: None,
        args: None,
        external_mcp_server: None,
        mcp_tool: None,
        context_window_tokens: None,
    }
}

#[test]
fn test_dispatch_payload_digest_covers_all_executor_inputs_deterministically() {
    use crate::orchestrator::types::{McpServerConfig, McpServerTransport, McpToolContract};

    let (_temp, project_path, _runs_dir) = setup_test_workspace();
    let plan_config = PlanWorkspaceConfig::default();
    let snapshot = capture_frozen_plan_snapshot(&project_path, &plan_config)
        .expect("capture frozen plan snapshot");
    let context = snapshot.plan_context.clone();
    let frozen_plan = snapshot
        .to_frozen_plan_payload()
        .expect("test workspace has a frozen plan payload");
    let role = AgentRole::Planner;
    let profile = sample_profile("planner", "Planner", "deepseek-r1");
    let system_prompt = "system prompt".to_string();
    let user_prompt = "task prompt".to_string();
    let mut servers_a = HashMap::new();
    let mut servers_b = HashMap::new();
    let server_a = McpServerConfig {
        transport: McpServerTransport::Stdio,
        executable: "server-a".into(),
        args: vec!["--mode".into(), "read".into()],
        working_directory: None,
        allowed_environment: vec![],
        tool_contract: McpToolContract::PromptEnvelopeV1,
    };
    let server_b = McpServerConfig {
        transport: McpServerTransport::Stdio,
        executable: "server-b".into(),
        args: vec![],
        working_directory: Some("workspace".into()),
        allowed_environment: vec!["PATH".into()],
        tool_contract: McpToolContract::PromptEnvelopeV1,
    };
    servers_a.insert("a".to_string(), server_a.clone());
    servers_a.insert("b".to_string(), server_b.clone());
    servers_b.insert("b".to_string(), server_b);
    servers_b.insert("a".to_string(), server_a);

    let digest = |role: &AgentRole,
                  profile: &OrchestratorProfile,
                  system_prompt: &str,
                  user_prompt: &str,
                  project_path: &Path,
                  servers: Option<&HashMap<String, McpServerConfig>>,
                  context: Option<&crate::orchestrator::plan_workspace::PlanContext>,
                  frozen: Option<&crate::orchestrator::plan_workspace::FrozenPlanPayload>| {
        super::driver::dispatch_payload_digest(
            role,
            profile,
            system_prompt,
            user_prompt,
            project_path,
            servers,
            context,
            frozen,
        )
        .expect("serialize complete dispatch identity")
    };

    let baseline = digest(
        &role,
        &profile,
        &system_prompt,
        &user_prompt,
        &project_path,
        Some(&servers_a),
        Some(&context),
        Some(&frozen_plan),
    );
    assert_eq!(
        baseline,
        digest(
            &role,
            &profile,
            &system_prompt,
            &user_prompt,
            &project_path,
            Some(&servers_a),
            Some(&context),
            Some(&frozen_plan),
        ),
        "identical calls must hash identically"
    );
    assert_eq!(
        baseline,
        digest(
            &role,
            &profile,
            &system_prompt,
            &user_prompt,
            &project_path,
            Some(&servers_b),
            Some(&context),
            Some(&frozen_plan),
        ),
        "HashMap insertion order must not affect the digest"
    );

    let changed_role = AgentRole::PlanReviewer;
    assert_ne!(
        baseline,
        digest(
            &changed_role,
            &profile,
            &system_prompt,
            &user_prompt,
            &project_path,
            Some(&servers_a),
            Some(&context),
            Some(&frozen_plan),
        )
    );
    let changed_profile = sample_profile("planner", "Planner", "different-model");
    assert_ne!(
        baseline,
        digest(
            &role,
            &changed_profile,
            &system_prompt,
            &user_prompt,
            &project_path,
            Some(&servers_a),
            Some(&context),
            Some(&frozen_plan),
        )
    );
    let mut changed_sampling = profile.clone();
    changed_sampling.reasoning_effort = Some("high".into());
    assert_ne!(
        baseline,
        digest(
            &role,
            &changed_sampling,
            &system_prompt,
            &user_prompt,
            &project_path,
            Some(&servers_a),
            Some(&context),
            Some(&frozen_plan),
        )
    );
    assert_ne!(
        baseline,
        digest(&role, &profile, "changed system", &user_prompt, &project_path, Some(&servers_a), Some(&context), Some(&frozen_plan))
    );
    assert_ne!(
        baseline,
        digest(&role, &profile, &system_prompt, "changed task", &project_path, Some(&servers_a), Some(&context), Some(&frozen_plan))
    );
    assert_ne!(
        baseline,
        digest(&role, &profile, &system_prompt, &user_prompt, &project_path.join("other"), Some(&servers_a), Some(&context), Some(&frozen_plan))
    );
    assert_ne!(
        baseline,
        digest(&role, &profile, &system_prompt, &user_prompt, &project_path, None, Some(&context), Some(&frozen_plan))
    );
    assert_ne!(
        baseline,
        digest(&role, &profile, &system_prompt, &user_prompt, &project_path, Some(&HashMap::new()), Some(&context), Some(&frozen_plan))
    );
    let mut changed_servers = servers_a.clone();
    changed_servers.get_mut("a").unwrap().executable = "changed-server".into();
    assert_ne!(
        baseline,
        digest(&role, &profile, &system_prompt, &user_prompt, &project_path, Some(&changed_servers), Some(&context), Some(&frozen_plan))
    );
    let mut changed_context = context.clone();
    changed_context.project_root_identity.push_str("-changed");
    assert_ne!(
        baseline,
        digest(&role, &profile, &system_prompt, &user_prompt, &project_path, Some(&servers_a), Some(&changed_context), Some(&frozen_plan))
    );
    let mut changed_plan = frozen_plan.clone();
    changed_plan.effective_plan_content.push_str(" changed");
    assert_ne!(
        baseline,
        digest(&role, &profile, &system_prompt, &user_prompt, &project_path, Some(&servers_a), Some(&context), Some(&changed_plan))
    );
}

#[test]
fn test_dispatch_payload_digest_conflict_rejected_for_same_id_when_non_prompt_input_changes() {
    use crate::orchestrator::recovery::{AdapterDispatchRecord, RunJournal};

    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path.clone(), runs_dir.clone(), true, 1);
    let snapshot = capture_frozen_plan_snapshot(&project_path, &PlanWorkspaceConfig::default())
        .expect("capture frozen plan snapshot");
    let context = snapshot.plan_context.clone();
    let frozen_plan = snapshot
        .to_frozen_plan_payload()
        .expect("test workspace has a frozen plan payload");
    let profile = sample_profile("planner-profile-1", "Planner", "deepseek-r1");
    let digest_for_context = |context: &crate::orchestrator::plan_workspace::PlanContext| {
        super::driver::dispatch_payload_digest(
            &AgentRole::Planner,
            &profile,
            "system",
            "same user prompt",
            &project_path,
            Some(&HashMap::new()),
            Some(context),
            Some(&frozen_plan),
        )
        .unwrap()
    };
    let original_digest = digest_for_context(&context);
    let mut changed_context = context;
    changed_context.project_root_identity.push_str("-changed");
    let changed_digest = digest_for_context(&changed_context);
    assert_ne!(original_digest, changed_digest);

    let manager = setup_journal_with_dispatches(&project_path, &runs_dir, &driver, vec![]);
    let mut journal: RunJournal = manager.read_journal(&driver.run_id).unwrap();
    let original = AdapterDispatchRecord {
        dispatch_id: format!("{}:1", driver.run_id),
        ordinal: 1,
        role: "planner".into(),
        profile_id: profile.id.clone(),
        model: profile.model.clone(),
        payload_digest: Some(original_digest),
        usage: None,
        status: Some("reserved".into()),
        created_at_unix: 1,
        completed_at_unix: None,
    };
    assert!(journal.record_adapter_dispatch(original.clone()).unwrap());
    let mut conflicting_retry = original;
    conflicting_retry.payload_digest = Some(changed_digest);
    let error = journal
        .record_adapter_dispatch(conflicting_retry)
        .expect_err("same dispatch ID with changed typed context must conflict");
    assert!(error.contains("PC_DISPATCH_CONFLICT"), "unexpected error: {error}");
}

fn sample_append_proposal_json(target_plan_id: &str) -> String {
    format!(
        r#"{{
  "schemaVersion": 1,
  "operation": {{
    "kind": "append_section",
    "targetPlanId": "{target_plan_id}",
    "sectionType": "implementation_notes",
    "sectionTitle": "New Section",
    "sectionContent": "Detailed step instructions."
  }}
}}"#
    )
}

fn sample_new_primary_proposal_json() -> String {
    r##"{
  "schemaVersion": 1,
  "operation": {
    "kind": "new_primary_plan",
    "proposedRevision": 2,
    "title": "Next Revision Plan",
    "initialContent": "# Revision 2 Plan\n\nNew primary plan content."
  }
}"##
    .to_string()
}

#[test]
fn test_operation_serialization_digest_and_reviewer_wire_use_identical_camel_case() {
    use super::parser::canonical_operation_bytes;
    let operations = [
        PlannerOperationProposal::AppendSection {
            target_plan_id: "V0.23.0-r1.md".into(),
            section_type: "implementation_notes".into(),
            section_title: "Title".into(),
            section_content: "Body".into(),
        },
        PlannerOperationProposal::NewPrimaryPlan {
            proposed_revision: 2,
            title: "Next".into(),
            initial_content: "Plan body".into(),
        },
    ];
    let expected = [
        r#"{"kind":"append_section","targetPlanId":"V0.23.0-r1.md","sectionType":"implementation_notes","sectionTitle":"Title","sectionContent":"Body"}"#,
        r#"{"kind":"new_primary_plan","proposedRevision":2,"title":"Next","initialContent":"Plan body"}"#,
    ];
    for (operation, expected_bytes) in operations.into_iter().zip(expected) {
        let bytes = canonical_operation_bytes(&operation);
        assert_eq!(bytes, serde_json::to_vec(&operation).unwrap());
        assert_eq!(bytes, expected_bytes.as_bytes());
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(value.get("kind").is_some());
        for key in [
            "targetPlanId",
            "sectionType",
            "sectionTitle",
            "sectionContent",
            "proposedRevision",
            "initialContent",
        ] {
            if value.get(key).is_some() {
                assert!(!key.contains('_'));
            }
        }
        let serialized = String::from_utf8(bytes).unwrap();
        assert!(!serialized.contains("target_plan_id"));
        assert!(!serialized.contains("section_content"));
        assert!(!serialized.contains("proposed_revision"));
    }
}

fn sample_approve_verdict_json(
    sequence: u32,
    candidate_id: &str,
    operation_payload_digest: &str,
    base_plan_digest: Option<&str>,
    plan_context_digest: &str,
) -> String {
    let base_plan_digest_str = match base_plan_digest {
        Some(d) => format!("\"basePlanDigest\": \"{d}\","),
        None => String::new(),
    };

    format!(
        r#"{{
  "schemaVersion": 1,
  "decision": "APPROVE",
  "reviewedCandidate": {{
    "sequence": {sequence},
    "candidateId": "{candidate_id}",
    "operationPayloadDigest": "{operation_payload_digest}",
    {base_plan_digest_str}
    "planContextDigest": "{plan_context_digest}"
  }},
  "summary": "Everything looks solid and meets all acceptance criteria.",
  "findings": []
}}"#
    )
}

fn sample_request_changes_verdict_json(
    sequence: u32,
    candidate_id: &str,
    operation_payload_digest: &str,
    base_plan_digest: Option<&str>,
    plan_context_digest: &str,
) -> String {
    let base_plan_digest_str = match base_plan_digest {
        Some(d) => format!("\"basePlanDigest\": \"{d}\","),
        None => String::new(),
    };

    format!(
        r#"{{
  "schemaVersion": 1,
  "decision": "REQUEST_CHANGES",
  "reviewedCandidate": {{
    "sequence": {sequence},
    "candidateId": "{candidate_id}",
    "operationPayloadDigest": "{operation_payload_digest}",
    {base_plan_digest_str}
    "planContextDigest": "{plan_context_digest}"
  }},
  "summary": "Needs additional detail in section 2.",
  "findings": [
    {{
      "findingId": "F-001",
      "affectedRequirement": "Section 2 completeness",
      "problem": "Missing error handling specifications.",
      "whyBlocking": "Errors must be explicitly handled.",
      "requiredChange": "Add explicit error recovery steps.",
      "scopeEffect": "NONE",
      "blocking": true
    }}
  ]
}}"#
    )
}

fn create_test_driver(
    project_path: PathBuf,
    runs_dir: PathBuf,
    opt_in: bool,
    max_reviews: u32,
) -> ConvergenceDriver {
    let plan_cfg = PlanWorkspaceConfig::default();
    let frozen_snapshot =
        capture_frozen_plan_snapshot(&project_path, &plan_cfg).expect("capture snapshot");

    ConvergenceDriver {
        runs_dir,
        run_id: "test-run-123".to_string(),
        project_path,
        task_prompt: "Implement feature X".to_string(),
        planner_profile: sample_profile("planner-profile-1", "Planner", "deepseek-r1"),
        reviewer_profile: sample_profile("reviewer-profile-1", "PlanReviewer", "gpt-4o"),
        plan_workspace_config: plan_cfg,
        frozen_plan_snapshot: frozen_snapshot,
        mcp_servers: HashMap::new(),
        max_plan_review_iterations: max_reviews,
        convergence_config: PlanConvergenceConfig {
            opt_in,
            total_timeout_secs: 60,
        },
    }
}

/// Scripted mock adapter executor for deterministic test execution
struct MockAdapterExecutor {
    planner_fn: Box<dyn Fn(u32, &str) -> Result<String, String> + Send + Sync>,
    reviewer_fn: Box<dyn Fn(u32, &str) -> Result<String, String> + Send + Sync>,
    planner_calls: Arc<AtomicUsize>,
    reviewer_calls: Arc<AtomicUsize>,
}

impl ConvergenceAdapterExecutor for MockAdapterExecutor {
    async fn execute_role(
        &self,
        role: AgentRole,
        _profile: &OrchestratorProfile,
        _system_prompt: &str,
        user_prompt: &str,
        _project_path: &Path,
        _mcp_servers: Option<&HashMap<String, crate::orchestrator::types::McpServerConfig>>,
        _plan_context: Option<&crate::orchestrator::plan_workspace::PlanContext>,
        _frozen_plan: Option<&crate::orchestrator::plan_workspace::FrozenPlanPayload>,
        _timeout: Duration,
        _cancel_token: Option<&CancellationToken>,
    ) -> Result<String, String> {
        match role {
            AgentRole::Planner => {
                let call_idx = self.planner_calls.fetch_add(1, Ordering::SeqCst) as u32;
                (self.planner_fn)(call_idx, user_prompt)
            }
            AgentRole::PlanReviewer => {
                let call_idx = self.reviewer_calls.fetch_add(1, Ordering::SeqCst) as u32;
                (self.reviewer_fn)(call_idx, user_prompt)
            }
            _ => Err("Unexpected role".to_string()),
        }
    }
}

fn parse_fields_from_reviewer_prompt(
    prompt: &str,
) -> (u32, String, String, Option<String>, String) {
    let mut seq = 1;
    let mut candidate_id = String::new();
    let mut op_digest = String::new();
    let mut base_digest = None;
    let mut ctx_digest = String::new();

    for line in prompt.lines() {
        if let Some(rest) = line.strip_prefix("Sequence: ") {
            seq = rest.trim().parse::<u32>().unwrap_or(1);
        } else if let Some(rest) = line.strip_prefix("Candidate ID: ") {
            candidate_id = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("Operation Payload Digest: ") {
            op_digest = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("Base Plan Digest: ") {
            let trimmed = rest.trim().trim_matches('"');
            if trimmed != "null" && !trimmed.is_empty() {
                base_digest = Some(trimmed.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("Plan Context Digest: ") {
            ctx_digest = rest.trim().to_string();
        }
    }
    (seq, candidate_id, op_digest, base_digest, ctx_digest)
}

// ===========================================================================
// Test 1: Wire Contracts & Strict Parser
// ===========================================================================

#[test]
fn test_wire_parser_strictness_and_fences() {
    // 1. Valid proposal
    let valid_json = sample_append_proposal_json("V0.23.0-r1.md");
    let parsed = parse_planner_proposal(&valid_json).expect("valid proposal");
    assert_eq!(parsed.schema_version, 1);
    match parsed.operation {
        PlannerOperationProposal::AppendSection {
            target_plan_id,
            section_type,
            section_title,
            section_content,
        } => {
            assert_eq!(target_plan_id, "V0.23.0-r1.md");
            assert_eq!(section_type, "implementation_notes");
            assert_eq!(section_title, "New Section");
            assert_eq!(section_content, "Detailed step instructions.");
        }
        _ => panic!("expected AppendSection"),
    }

    // 2. Rejects markdown fences
    let fenced = format!("```json\n{valid_json}\n```");
    let err = parse_planner_proposal(&fenced).unwrap_err();
    assert_eq!(err.code, "PC_INVALID_PROPOSAL");
    assert!(err.message.contains("markdown code fences"));

    // 3. Rejects unknown fields
    let with_unknown = r#"{
  "schemaVersion": 1,
  "unknownField": "bad",
  "operation": {
    "kind": "append_section",
    "targetPlanId": "V0.23.0-r1.md",
    "sectionType": "note",
    "sectionTitle": "Title",
    "sectionContent": "Content"
  }
}"#;
    let err = parse_planner_proposal(with_unknown).unwrap_err();
    assert_eq!(err.code, "PC_INVALID_PROPOSAL");
    assert!(err.message.contains("Unknown top-level field"));

    // 4. Rejects invalid schema version
    let bad_schema = r#"{
  "schemaVersion": 99,
  "operation": {
    "kind": "append_section",
    "targetPlanId": "V0.23.0-r1.md",
    "sectionType": "note",
    "sectionTitle": "Title",
    "sectionContent": "Content"
  }
}"#;
    let err = parse_planner_proposal(bad_schema).unwrap_err();
    assert_eq!(err.code, "PC_INVALID_PROPOSAL");

    // 5. Rejects oversized proposal
    let huge_content = "a".repeat(MAX_PAYLOAD_BYTES + 10);
    let huge_json = format!(
        r#"{{
  "schemaVersion": 1,
  "operation": {{
    "kind": "append_section",
    "targetPlanId": "V0.23.0-r1.md",
    "sectionType": "note",
    "sectionTitle": "Title",
    "sectionContent": "{huge_content}"
  }}
}}"#
    );
    let err = parse_planner_proposal(&huge_json).unwrap_err();
    assert_eq!(err.code, "PC_CANDIDATE_OVERSIZED");
}

#[test]
fn test_verdict_parser_strictness_and_duplicate_findings() {
    let base_verdict = format!(
        r#"{{
  "schemaVersion": 1,
  "decision": "REQUEST_CHANGES",
  "reviewedCandidate": {{
    "sequence": 1,
    "candidateId": "{DUMMY_UUID}",
    "operationPayloadDigest": "{DUMMY_HEX_64}",
    "basePlanDigest": "{DUMMY_HEX_64}",
    "planContextDigest": "{DUMMY_HEX_64}"
  }},
  "summary": "Has duplicates",
  "findings": [
    {{
      "findingId": "F-001",
      "affectedRequirement": "R1",
      "problem": "P1",
      "whyBlocking": "W1",
      "requiredChange": "C1",
      "scopeEffect": "NONE",
      "blocking": true
    }},
    {{
      "findingId": "F-001",
      "affectedRequirement": "R2",
      "problem": "P2",
      "whyBlocking": "W2",
      "requiredChange": "C2",
      "scopeEffect": "NONE",
      "blocking": true
    }}
  ]
}}"#
    );

    let err = parse_reviewer_proposal(&base_verdict).unwrap_err();
    assert_eq!(err.code, "PC_INVALID_VERDICT");
    assert!(err.message.contains("Duplicate finding_id 'F-001'"));
}

#[test]
fn test_planner_parser_rejects_duplicate_decoded_keys_and_noncanonical_aliases() {
    let duplicate_top = r#"{"schemaVersion":1,"schema\u0056ersion":1,"operation":{}}"#;
    let duplicate_nested = r#"{"schemaVersion":1,"operation":{"kind":"append_section","targetPlanId":"V0.23.0-r1.md","sectionType":"note","sectionTitle":"T","sectionContent":"C","extra":[{"x":1,"\u0078":2}]}}"#;
    let snake_case = r#"{"schema_version":1,"operation":{"kind":"append_section","target_plan_id":"V0.23.0-r1.md","section_type":"note","section_title":"T","section_content":"C"}}"#;
    let canonical_and_alias = r#"{"schemaVersion":1,"schema_version":1,"operation":{"kind":"append_section","targetPlanId":"V0.23.0-r1.md","sectionType":"note","sectionTitle":"T","sectionContent":"C"}}"#;

    for raw in [
        duplicate_top,
        duplicate_nested,
        snake_case,
        canonical_and_alias,
    ] {
        assert_eq!(
            parse_planner_proposal(raw).unwrap_err().code,
            "PC_INVALID_PROPOSAL",
            "input: {raw}"
        );
    }
}

#[test]
fn test_reviewer_parser_rejects_duplicate_decoded_keys_and_noncanonical_aliases() {
    let valid = sample_approve_verdict_json(1, DUMMY_UUID, DUMMY_HEX_64, None, DUMMY_HEX_64);
    let duplicate_top = valid.replace(
        "\"schemaVersion\": 1",
        "\"schemaVersion\": 1, \"schema\\u0056ersion\": 1",
    );
    let duplicate_nested = valid.replace(
        "\"findings\": []",
        "\"findings\": [{\"findingId\":\"F1\",\"finding\\u0049d\":\"F1\"}]",
    );
    let snake_case = valid.replace("\"schemaVersion\"", "\"schema_version\"");
    let nested_snake_case = valid.replace("\"candidateId\"", "\"candidate_id\"");
    let canonical_and_alias = valid.replace(
        "\"schemaVersion\": 1",
        "\"schemaVersion\": 1, \"schema_version\": 1",
    );

    for raw in [
        duplicate_top,
        duplicate_nested,
        snake_case,
        nested_snake_case,
        canonical_and_alias,
    ] {
        assert_eq!(
            parse_reviewer_proposal(&raw).unwrap_err().code,
            "PC_INVALID_VERDICT",
            "input: {raw}"
        );
    }
}

// ===========================================================================
// Test 2 & 3: Review Binding & Decision Semantics
// ===========================================================================

#[test]
fn test_review_verdict_decision_semantics_and_binding() {
    let (_temp, project_path, _runs_dir) = setup_test_workspace();
    let plan_cfg = PlanWorkspaceConfig::default();
    let context = resolve_plan_context(&project_path, &plan_cfg).expect("resolve context");

    let op = PlannerOperationProposal::AppendSection {
        target_plan_id: "V0.23.0-r1.md".to_string(),
        section_type: "implementation_notes".to_string(),
        section_title: "New Section".to_string(),
        section_content: "Content".to_string(),
    };
    let op_bytes = canonical_operation_bytes(&op);
    let op_digest = sha256_hex(&op_bytes);

    let candidate_id = Uuid::new_v4().to_string();
    let candidate = PlanCandidate {
        schema_version: 1,
        run_id: "test-run-123".to_string(),
        sequence: 1,
        candidate_id: candidate_id.clone(),
        operation: op,
        target_plan_id: Some("V0.23.0-r1.md".to_string()),
        base_plan_digest: context
            .current_primary_plan
            .as_ref()
            .map(|p| p.digest.clone()),
        plan_context_digest: context.effective_plan_digest.clone().unwrap(),
        operation_payload_digest: op_digest.clone(),
        producer_role: "Planner".to_string(),
        producer_profile_id: "p-1".to_string(),
        producer_model: "deepseek-r1".to_string(),
        created_at_unix: 1000,
    };

    // 1. APPROVE with exact bindings -> PolicyGate PASS
    let approve_json = sample_approve_verdict_json(
        1,
        &candidate.candidate_id,
        &candidate.operation_payload_digest,
        candidate.base_plan_digest.as_deref(),
        &candidate.plan_context_digest,
    );
    let approve_proposal = parse_reviewer_proposal(&approve_json).expect("parse approve");
    let approve_verdict = ReviewVerdict {
        schema_version: approve_proposal.schema_version,
        decision: approve_proposal.decision,
        reviewed_candidate: approve_proposal.reviewed_candidate,
        summary: approve_proposal.summary,
        findings: approve_proposal.findings,
        reviewer_role: "PlanReviewer".to_string(),
        reviewer_profile_id: "r-1".to_string(),
        reviewer_model: "gpt-4o".to_string(),
        duration_ms: 100,
    };

    let gate_res = PolicyGate::evaluate(
        &candidate,
        &approve_verdict,
        &context,
        true,
        "p-1",
        "r-1",
        1,
        3,
        false,
    )
    .expect("gate evaluate");
    assert!(matches!(
        gate_res,
        PolicyGateAction::AcceptAppendSection { .. }
    ));

    // 2. APPROVE with blocking finding -> Rejected by parser
    let approve_with_blocking = format!(
        r#"{{
  "schemaVersion": 1,
  "decision": "APPROVE",
  "reviewedCandidate": {{
    "sequence": 1,
    "candidateId": "{candidate_id}",
    "operationPayloadDigest": "{op_digest}",
    "basePlanDigest": "{}",
    "planContextDigest": "{}"
  }},
  "summary": "Looks good but blocking",
  "findings": [
    {{
      "findingId": "F-001",
      "affectedRequirement": "Req",
      "problem": "Problem",
      "whyBlocking": "Blocking reason",
      "requiredChange": "Change",
      "scopeEffect": "NONE",
      "blocking": true
    }}
  ]
}}"#,
        candidate.base_plan_digest.as_deref().unwrap(),
        candidate.plan_context_digest
    );
    let parse_err = parse_reviewer_proposal(&approve_with_blocking).unwrap_err();
    assert_eq!(parse_err.code, "PC_INVALID_VERDICT");
    assert!(parse_err
        .message
        .contains("APPROVE decision cannot contain blocking findings"));

    // 3. REQUEST_CHANGES without actionable blocking finding -> Rejected by parser
    let req_changes_no_blocking = format!(
        r#"{{
  "schemaVersion": 1,
  "decision": "REQUEST_CHANGES",
  "reviewedCandidate": {{
    "sequence": 1,
    "candidateId": "{candidate_id}",
    "operationPayloadDigest": "{op_digest}",
    "basePlanDigest": "{}",
    "planContextDigest": "{}"
  }},
  "summary": "Please change",
  "findings": [
    {{
      "findingId": "F-001",
      "affectedRequirement": "Req",
      "problem": "Problem",
      "whyBlocking": "None",
      "requiredChange": "Change",
      "scopeEffect": "NONE",
      "blocking": false
    }}
  ]
}}"#,
        candidate.base_plan_digest.as_deref().unwrap(),
        candidate.plan_context_digest
    );
    let parse_err2 = parse_reviewer_proposal(&req_changes_no_blocking).unwrap_err();
    assert_eq!(parse_err2.code, "PC_INVALID_VERDICT");
    assert!(parse_err2
        .message
        .contains("REQUEST_CHANGES decision must contain at least one blocking finding"));

    // 4. Mismatched candidate ID binding -> Gate fails
    let other_uuid = Uuid::new_v4().to_string();
    let mismatched_verdict = ReviewVerdict {
        schema_version: 1,
        decision: ReviewDecision::Approve,
        reviewed_candidate: ReviewedCandidateBinding {
            sequence: 1,
            candidate_id: other_uuid,
            operation_payload_digest: candidate.operation_payload_digest.clone(),
            base_plan_digest: candidate.base_plan_digest.clone(),
            plan_context_digest: candidate.plan_context_digest.clone(),
        },
        summary: "Approved".to_string(),
        findings: vec![],
        reviewer_role: "PlanReviewer".to_string(),
        reviewer_profile_id: "r-1".to_string(),
        reviewer_model: "gpt-4o".to_string(),
        duration_ms: 50,
    };
    let gate_err = PolicyGate::evaluate(
        &candidate,
        &mismatched_verdict,
        &context,
        true,
        "p-1",
        "r-1",
        1,
        3,
        false,
    )
    .unwrap_err();
    assert_eq!(gate_err.code, "PC_STALE_BINDING");
}

// ===========================================================================
// Test 4 & 5: Operation Safety & NewPrimaryPlan Human Hand-off
// ===========================================================================

#[tokio::test]
async fn test_new_primary_plan_routes_to_waiting_for_user() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path.clone(), runs_dir, true, 3);

    let new_plan_json = sample_new_primary_proposal_json();

    let executor = MockAdapterExecutor {
        planner_fn: Box::new(move |_, _| Ok(new_plan_json.clone())),
        reviewer_fn: Box::new(move |_, prompt| {
            let (seq, c_id, op_dig, base_dig, ctx_dig) = parse_fields_from_reviewer_prompt(prompt);
            Ok(sample_approve_verdict_json(
                seq,
                &c_id,
                &op_dig,
                base_dig.as_deref(),
                &ctx_dig,
            ))
        }),
        planner_calls: Arc::new(AtomicUsize::new(0)),
        reviewer_calls: Arc::new(AtomicUsize::new(0)),
    };

    let audit_trail = Arc::new(Mutex::new(Vec::new()));
    let audit_clone = Arc::clone(&audit_trail);
    let audit_cb: AuditPersistenceFn = Arc::new(move |entry| {
        audit_clone.lock().unwrap().push(entry);
        Ok(())
    });

    let outcome = driver.run(&executor, audit_cb, None, None).await;

    // Must be WaitingForUser with NEW_PRIMARY_PLAN_CONFIRMATION
    match outcome {
        ConvergenceOutcome::WaitingForUser {
            reason,
            diagnostic_code,
            candidate_artifact_ref,
            latest_verdict_artifact_ref,
            ..
        } => {
            assert_eq!(reason, ConvergenceWaitingReason::NewPrimaryPlanConfirmation);
            assert_eq!(
                diagnostic_code.as_deref(),
                Some("PC_HUMAN_CONFIRMATION_REQUIRED")
            );
            assert!(candidate_artifact_ref.is_some());
            assert!(latest_verdict_artifact_ref.is_some());
        }
        other => panic!("expected WaitingForUser, got {other:?}"),
    }

    // Verify workspace file was NOT automatically mutated
    let current = plan_current(&project_path, &driver.plan_workspace_config).expect("plan_current");
    assert_eq!(
        current.context.current_primary_plan.unwrap().id,
        "V0.23.0-r1"
    );
}

// ===========================================================================
// Test 6 & 7: Budget Limits & Distinct Role Requirement
// ===========================================================================

#[tokio::test]
async fn test_distinct_role_and_budget_limits() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();

    // 1. Same role/profile error
    let mut bad_driver = create_test_driver(project_path.clone(), runs_dir.clone(), true, 3);
    bad_driver.reviewer_profile.id = bad_driver.planner_profile.id.clone();

    let dummy_executor = MockAdapterExecutor {
        planner_fn: Box::new(|_, _| Ok(String::new())),
        reviewer_fn: Box::new(|_, _| Ok(String::new())),
        planner_calls: Arc::new(AtomicUsize::new(0)),
        reviewer_calls: Arc::new(AtomicUsize::new(0)),
    };

    let audit_cb: AuditPersistenceFn = Arc::new(|_| Ok(()));
    let outcome = bad_driver
        .run(&dummy_executor, audit_cb.clone(), None, None)
        .await;
    match outcome {
        ConvergenceOutcome::Failed {
            stable_error_code, ..
        } => {
            assert_eq!(stable_error_code, "PC_DISTINCT_ROLES_REQUIRED");
        }
        other => panic!("expected Failed, got {other:?}"),
    }

    // 2. Budget limits (max 2 review iterations)
    let driver = create_test_driver(project_path.clone(), runs_dir, true, 2);

    let p_json = sample_append_proposal_json("V0.23.0-r1.md");
    let executor = MockAdapterExecutor {
        planner_fn: Box::new(move |_, _| Ok(p_json.clone())),
        reviewer_fn: Box::new(move |_, prompt| {
            let (seq, c_id, op_dig, base_dig, ctx_dig) = parse_fields_from_reviewer_prompt(prompt);
            // Always request changes
            Ok(sample_request_changes_verdict_json(
                seq,
                &c_id,
                &op_dig,
                base_dig.as_deref(),
                &ctx_dig,
            ))
        }),
        planner_calls: Arc::new(AtomicUsize::new(0)),
        reviewer_calls: Arc::new(AtomicUsize::new(0)),
    };

    let outcome2 = driver.run(&executor, audit_cb, None, None).await;
    match outcome2 {
        ConvergenceOutcome::WaitingForUser {
            reason,
            diagnostic_code,
            ..
        } => {
            assert_eq!(reason, ConvergenceWaitingReason::ReviewLimitReached);
            assert_eq!(diagnostic_code.as_deref(), Some("PC_REVIEW_LIMIT_REACHED"));
        }
        other => panic!("expected WaitingForUser with ReviewLimitReached, got {other:?}"),
    }
}

// ===========================================================================
// Test 8: End-to-End Multi-Cycle Convergence (Request Changes -> Approve)
// ===========================================================================

#[tokio::test]
async fn test_e2e_multi_cycle_convergence() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path.clone(), runs_dir.clone(), true, 3);

    let p_json_1 = sample_append_proposal_json("V0.23.0-r1.md");
    let p_json_2 = sample_append_proposal_json("V0.23.0-r1.md");

    let executor = MockAdapterExecutor {
        planner_fn: Box::new(move |call_idx, _| {
            if call_idx == 0 {
                Ok(p_json_1.clone())
            } else {
                Ok(p_json_2.clone())
            }
        }),
        reviewer_fn: Box::new(move |call_idx, prompt| {
            let (seq, c_id, op_dig, base_dig, ctx_dig) = parse_fields_from_reviewer_prompt(prompt);
            if call_idx == 0 {
                Ok(sample_request_changes_verdict_json(
                    seq,
                    &c_id,
                    &op_dig,
                    base_dig.as_deref(),
                    &ctx_dig,
                ))
            } else {
                Ok(sample_approve_verdict_json(
                    seq,
                    &c_id,
                    &op_dig,
                    base_dig.as_deref(),
                    &ctx_dig,
                ))
            }
        }),
        planner_calls: Arc::new(AtomicUsize::new(0)),
        reviewer_calls: Arc::new(AtomicUsize::new(0)),
    };

    let audit_trail = Arc::new(Mutex::new(Vec::new()));
    let audit_clone = Arc::clone(&audit_trail);
    let audit_cb: AuditPersistenceFn = Arc::new(move |entry| {
        if entry.event_type != "dispatch_reserved"
            && entry.event_type != "dispatch_failed"
            && entry.event_type != "dispatch_completed"
        {
            audit_clone.lock().unwrap().push(entry);
        }
        Ok(())
    });

    let outcome = driver.run(&executor, audit_cb, None, None).await;

    match outcome {
        ConvergenceOutcome::Accepted {
            candidate_id,
            target_plan_id,
            accepted_file_digest,
            reviews_used,
            ..
        } => {
            assert_eq!(target_plan_id, "V0.23.0-r1.md");
            assert_eq!(reviews_used, 2);
            assert_eq!(accepted_file_digest.len(), 64);
            assert!(!candidate_id.is_empty());
        }
        other => panic!("expected Accepted, got {other:?}"),
    }

    // Verify workspace file now contains the appended section
    let plan_content =
        fs::read_to_string(project_path.join(".plan").join("V0.23.0-r1.md")).expect("read plan");
    assert!(plan_content.contains("## New Section"));
    assert!(plan_content.contains("Detailed step instructions."));

    // Verify artifact integrity verification
    let entries = audit_trail.lock().unwrap().clone();
    assert_eq!(entries.len(), 9); // frozen snapshot, two candidate/review cycles, policy approval, acceptance
    load_and_verify_run_artifacts(&runs_dir, &driver.run_id, &entries)
        .expect("verify run artifacts");
    let candidate_entry = entries
        .iter()
        .rev()
        .find(|entry| entry.event_type == "candidate_created")
        .unwrap();
    let candidate = super::artifacts::load_and_verify_candidate(
        &runs_dir,
        &driver.run_id,
        candidate_entry.artifact_ref.as_deref().unwrap(),
        candidate_entry.artifact_digest.as_deref().unwrap(),
    )
    .unwrap();
    let candidate_path = runs_dir
        .join(&driver.run_id)
        .join(candidate_entry.artifact_ref.as_deref().unwrap());
    let candidate_wire: serde_json::Value =
        serde_json::from_slice(&fs::read(candidate_path).unwrap()).unwrap();
    assert_eq!(
        candidate_wire["operation"]["sectionContent"],
        "Detailed step instructions."
    );
    assert!(candidate_wire["operation"].get("section_content").is_none());
    assert_eq!(
        canonical_operation_bytes(&candidate.operation),
        serde_json::to_vec(&candidate.operation).unwrap()
    );
    let verdict_ref = entries
        .iter()
        .find(|entry| entry.event_type == "verdict_recorded")
        .unwrap()
        .artifact_ref
        .as_ref()
        .unwrap();
    fs::write(
        runs_dir.join(&driver.run_id).join(verdict_ref),
        b"corrupt verdict",
    )
    .unwrap();
    assert!(
        load_and_verify_run_artifacts(&runs_dir, &driver.run_id, &entries).is_err(),
        "review artifacts in artifacts/reviews must actually be verified"
    );
}

#[tokio::test]
async fn test_new_run_with_identical_payload_is_a_distinct_append_after_prior_audit_crash() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let first_driver = create_test_driver(project_path.clone(), runs_dir.clone(), true, 2);
    let proposal = sample_append_proposal_json("V0.23.0-r1.md");
    let make_executor = || {
        let proposal = proposal.clone();
        MockAdapterExecutor {
            planner_fn: Box::new(move |_, _| Ok(proposal.clone())),
            reviewer_fn: Box::new(|_, prompt| {
                let (seq, candidate, op, base, context) = parse_fields_from_reviewer_prompt(prompt);
                Ok(sample_approve_verdict_json(
                    seq,
                    &candidate,
                    &op,
                    base.as_deref(),
                    &context,
                ))
            }),
            planner_calls: Arc::new(AtomicUsize::new(0)),
            reviewer_calls: Arc::new(AtomicUsize::new(0)),
        }
    };
    let audit: AuditPersistenceFn = Arc::new(|entry| {
        if entry.event_type == "acceptance_succeeded" {
            Err("simulated crash after append".into())
        } else {
            Ok(())
        }
    });
    assert!(
        matches!(first_driver.run(&make_executor(), audit, None, None).await,
        ConvergenceOutcome::Failed { stable_error_code, .. } if stable_error_code == "PC_PERSISTENCE_FAILED")
    );
    let plan_path = project_path.join(".plan/V0.23.0-r1.md");
    let after_first = fs::read_to_string(&plan_path).unwrap();
    assert_eq!(after_first.matches("<!-- idempotency_token:").count(), 1);

    // New run ID and runtime candidate ID: identical content is a distinct
    // authorized operation, not a replay of the first candidate transaction.
    let mut second_driver = create_test_driver(project_path.clone(), runs_dir, true, 2);
    second_driver.run_id = "test-run-after-crash".to_string();
    assert_ne!(first_driver.run_id, second_driver.run_id);
    assert!(matches!(
        second_driver
            .run(&make_executor(), Arc::new(|_| Ok(())), None, None)
            .await,
        ConvergenceOutcome::Accepted { .. }
    ));
    let after_second = fs::read_to_string(plan_path).unwrap();
    assert_eq!(after_second.matches("<!-- idempotency_token:").count(), 2);
    assert_eq!(
        after_second.matches("Detailed step instructions.").count(),
        2
    );
}

#[tokio::test]
async fn test_driver_reuses_the_same_frozen_plan_body_after_live_file_changes() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let plan_path = project_path.join(".plan/V0.23.0-r1.md");
    let planner_prompts = Arc::new(Mutex::new(Vec::<String>::new()));
    let reviewer_prompts = Arc::new(Mutex::new(Vec::<String>::new()));
    let planner_capture = Arc::clone(&planner_prompts);
    let reviewer_capture = Arc::clone(&reviewer_prompts);
    let plan_for_planner = plan_path.clone();
    let proposal = sample_append_proposal_json("V0.23.0-r1.md");
    let executor = MockAdapterExecutor {
        planner_fn: Box::new(move |_, prompt| {
            let start = prompt
                .find("<run_frozen_plan_context_v1>")
                .expect("frozen context start");
            let end = prompt
                .find("</run_frozen_plan_context_v1>")
                .expect("frozen context end")
                + "</run_frozen_plan_context_v1>".len();
            planner_capture
                .lock()
                .unwrap()
                .push(prompt[start..end].to_string());
            fs::write(&plan_for_planner, "# Plan changed after run start\n").unwrap();
            Ok(proposal.clone())
        }),
        reviewer_fn: Box::new(move |_, prompt| {
            let start = prompt
                .find("<run_frozen_plan_context_v1>")
                .expect("frozen context start");
            let end = prompt
                .find("</run_frozen_plan_context_v1>")
                .expect("frozen context end")
                + "</run_frozen_plan_context_v1>".len();
            reviewer_capture
                .lock()
                .unwrap()
                .push(prompt[start..end].to_string());
            assert!(prompt.contains(r#"{"kind":"append_section","targetPlanId":"V0.23.0-r1.md","sectionType":"implementation_notes","sectionTitle":"New Section","sectionContent":"Detailed step instructions."}"#));
            assert!(prompt.contains("Initial plan content."));
            assert!(!prompt.contains("Plan changed after run start"));
            let (seq, candidate, op, base, context) = parse_fields_from_reviewer_prompt(prompt);
            Ok(sample_approve_verdict_json(
                seq,
                &candidate,
                &op,
                base.as_deref(),
                &context,
            ))
        }),
        planner_calls: Arc::new(AtomicUsize::new(0)),
        reviewer_calls: Arc::new(AtomicUsize::new(0)),
    };
    let driver = create_test_driver(project_path, runs_dir, true, 2);
    let audit: Arc<Mutex<Vec<PlanConvergenceAuditEntry>>> = Arc::new(Mutex::new(Vec::new()));
    let audit_copy = Arc::clone(&audit);
    let outcome = driver
        .run(
            &executor,
            Arc::new(move |entry| {
                audit_copy.lock().unwrap().push(entry);
                Ok(())
            }),
            None,
            None,
        )
        .await;

    assert!(
        matches!(
            &outcome,
            ConvergenceOutcome::WaitingForUser {
                reason: ConvergenceWaitingReason::PlanContextStale,
                ..
            }
        ),
        "expected fail-closed stale-context wait after reviewer, got {outcome:?}"
    );
    let planner = planner_prompts.lock().unwrap();
    let reviewer = reviewer_prompts.lock().unwrap();
    assert_eq!(planner.len(), 1);
    assert_eq!(reviewer.len(), 1);
    assert_eq!(
        planner[0], reviewer[0],
        "all ordinary adapter roles must receive the same frozen bytes and digests"
    );
    assert!(planner[0].contains("Initial plan content."));
    assert_eq!(
        audit.lock().unwrap().first().unwrap().event_type,
        "frozen_plan_snapshot_recorded"
    );
}

#[tokio::test]
async fn test_engine_adapter_dispatch_preserves_frozen_plan_for_ordinary_adapters() {
    use crate::orchestrator::adapters::{AdapterExecutionInput, AdapterExecutionOutput};
    use crate::orchestrator::engine::OrchestratorEngine;

    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let plan_path = project_path.join(".plan/V0.23.0-r1.md");
    let supplemental_path = project_path.join(".plan/V0.23.0-r1a.md");
    let original_primary = "# Primary plan\nORIGINAL_PRIMARY_BYTES\n";
    let original_supplemental = "# Supplemental plan\nORIGINAL_SUPPLEMENTAL_BYTES\n";
    fs::write(&plan_path, original_primary).unwrap();
    fs::write(&supplemental_path, original_supplemental).unwrap();

    for adapter in [
        ExecutionAdapterType::Provider,
        ExecutionAdapterType::Ollama,
        ExecutionAdapterType::Cli,
    ] {
        // Each adapter run starts from identical live plan bytes and gets a
        // distinct durable artifact directory.
        fs::write(&plan_path, original_primary).unwrap();
        fs::write(&supplemental_path, original_supplemental).unwrap();
        let mut driver = create_test_driver(project_path.clone(), runs_dir.clone(), true, 2);
        driver.run_id = format!("ordinary-adapter-{adapter:?}");
        driver.planner_profile.adapter = adapter.clone();
        driver.reviewer_profile.adapter = adapter.clone();
        driver
            .reviewer_profile
            .capabilities
            .push(ProfileCapability::Review);

        let frozen_before_dispatch = driver.frozen_plan_snapshot.clone();
        let expected_payload = frozen_before_dispatch
            .to_frozen_plan_payload()
            .expect("resolved run snapshot must have a payload");
        let captured = Arc::new(Mutex::new(Vec::<(
            ExecutionAdapterType,
            AdapterExecutionInput,
        )>::new()));
        let captured_dispatch = Arc::clone(&captured);
        let primary_for_mutation = plan_path.clone();
        let supplemental_for_mutation = supplemental_path.clone();
        let responder: Arc<
            dyn Fn(
                    ExecutionAdapterType,
                    &AdapterExecutionInput,
                ) -> Result<AdapterExecutionOutput, String>
                + Send
                + Sync,
        > = Arc::new(move |selected_adapter, input| {
            captured_dispatch
                .lock()
                .unwrap()
                .push((selected_adapter, input.clone()));
            let content = match input.role {
                AgentRole::Planner => {
                    fs::write(
                        &primary_for_mutation,
                        "# LIVE MUTATION\nMUTATED_PRIMARY_BYTES\n",
                    )
                    .map_err(|error| error.to_string())?;
                    fs::write(
                        &supplemental_for_mutation,
                        "# LIVE MUTATION\nMUTATED_SUPPLEMENTAL_BYTES\n",
                    )
                    .map_err(|error| error.to_string())?;
                    sample_append_proposal_json("V0.23.0-r1.md")
                }
                AgentRole::PlanReviewer => {
                    let (sequence, candidate, operation, base, context) =
                        parse_fields_from_reviewer_prompt(&input.user_prompt);
                    sample_approve_verdict_json(
                        sequence,
                        &candidate,
                        &operation,
                        base.as_deref(),
                        &context,
                    )
                }
                _ => return Err(format!("unexpected convergence role: {:?}", input.role)),
            };
            Ok(AdapterExecutionOutput {
                content,
                raw_json: None,
                tokens_used: None,
                model_used: "dispatch-test-double".into(),
                duration_ms: 0,
                usage: None,
            })
        });
        let engine = OrchestratorEngine::new().with_ordinary_adapter_test_dispatch(responder);
        let executor = EngineAdapterExecutor { engine };
        let outcome = driver
            .run(&executor, Arc::new(|_| Ok(())), None, None)
            .await;

        assert!(
            matches!(
                outcome,
                ConvergenceOutcome::WaitingForUser {
                    reason: ConvergenceWaitingReason::PlanContextStale,
                    ..
                }
            ),
            "{adapter:?} must fail closed after the live workspace changes, got {outcome:?}"
        );
        let calls = captured.lock().unwrap();
        assert_eq!(
            calls.len(),
            2,
            "{adapter:?} must reach both production dispatch branches"
        );
        assert_eq!(calls[0].0, adapter);
        assert_eq!(calls[1].0, adapter);
        assert_eq!(calls[0].1.role, AgentRole::Planner);
        assert_eq!(calls[1].1.role, AgentRole::PlanReviewer);

        let frozen_block = |prompt: &str| -> String {
            let start = prompt
                .find("<run_frozen_plan_context_v1>")
                .expect("production adapter input must contain frozen context");
            let end_marker = "</run_frozen_plan_context_v1>";
            let end =
                prompt.find(end_marker).expect("frozen context closing tag") + end_marker.len();
            prompt[start..end].to_string()
        };
        let planner_frozen = frozen_block(&calls[0].1.user_prompt);
        let reviewer_frozen = frozen_block(&calls[1].1.user_prompt);
        assert_eq!(
            planner_frozen, reviewer_frozen,
            "{adapter:?} roles must receive byte-identical frozen context"
        );
        let open_tag = "<run_frozen_plan_context_v1>";
        let close_tag = "</run_frozen_plan_context_v1>";
        let frozen_json = &reviewer_frozen[open_tag.len()..reviewer_frozen.len() - close_tag.len()];
        let frozen_wire: serde_json::Value = serde_json::from_str(frozen_json).unwrap();
        assert_eq!(
            frozen_wire["planContext"]["effectivePlanDigest"],
            frozen_before_dispatch
                .plan_context
                .effective_plan_digest
                .as_deref()
                .unwrap()
        );
        assert_eq!(
            frozen_wire["frozenPlanPayload"]["effectivePlanDigest"],
            expected_payload.effective_plan_digest
        );
        assert_eq!(
            frozen_wire["frozenPlanPayload"]["contentDigest"],
            expected_payload.content_digest
        );
        let sources = frozen_wire["frozenPlanPayload"]["orderedSources"]
            .as_array()
            .unwrap();
        assert_eq!(
            sources.len(),
            2,
            "primary and supplemental source order must be preserved"
        );
        assert_eq!(sources[0]["content"], original_primary);
        assert_eq!(sources[1]["content"], original_supplemental);
        assert!(calls[1].1.user_prompt.contains(
            r#"{"kind":"append_section","targetPlanId":"V0.23.0-r1.md","sectionType":"implementation_notes","sectionTitle":"New Section","sectionContent":"Detailed step instructions."}"#
        ));
        for (_, input) in calls.iter() {
            assert!(!input.user_prompt.contains("MUTATED_PRIMARY_BYTES"));
            assert!(!input.user_prompt.contains("MUTATED_SUPPLEMENTAL_BYTES"));
        }
    }
}

#[tokio::test]
async fn test_invalid_or_oversized_frozen_plan_never_reaches_ordinary_adapter_dispatch() {
    use crate::orchestrator::adapters::{AdapterExecutionInput, AdapterExecutionOutput};
    use crate::orchestrator::engine::OrchestratorEngine;

    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let dispatched = Arc::new(AtomicUsize::new(0));
    let dispatch_count = Arc::clone(&dispatched);
    let engine = OrchestratorEngine::new().with_ordinary_adapter_test_dispatch(Arc::new(
        move |_: ExecutionAdapterType, _: &AdapterExecutionInput| {
            dispatch_count.fetch_add(1, Ordering::SeqCst);
            Ok(AdapterExecutionOutput {
                content: String::new(),
                raw_json: None,
                tokens_used: None,
                model_used: "dispatch-test-double".into(),
                duration_ms: 0,
                usage: None,
            })
        },
    ));
    let executor = EngineAdapterExecutor { engine };

    let mut missing = create_test_driver(project_path.clone(), runs_dir.clone(), true, 1);
    missing.frozen_plan_snapshot.effective_plan_content = None;
    let missing_outcome = missing
        .run(&executor, Arc::new(|_| Ok(())), None, None)
        .await;
    assert!(
        matches!(missing_outcome, ConvergenceOutcome::Failed { stable_error_code, .. } if stable_error_code == "PC_INVALID_FROZEN_PLAN")
    );
    assert_eq!(
        dispatched.load(Ordering::SeqCst),
        0,
        "missing payload must fail before dispatch"
    );

    let mut mismatched = create_test_driver(project_path.clone(), runs_dir.clone(), true, 1);
    mismatched
        .frozen_plan_snapshot
        .plan_context
        .effective_plan_digest = Some("f".repeat(64));
    let mismatch_outcome = mismatched
        .run(&executor, Arc::new(|_| Ok(())), None, None)
        .await;
    assert!(
        matches!(mismatch_outcome, ConvergenceOutcome::Failed { stable_error_code, .. } if stable_error_code == "PC_INVALID_FROZEN_PLAN")
    );
    assert_eq!(
        dispatched.load(Ordering::SeqCst),
        0,
        "digest mismatch must fail before dispatch"
    );

    let plan_path = project_path.join(".plan/V0.23.0-r1.md");
    fs::write(
        &plan_path,
        format!("# Oversized plan\n{}", "x".repeat(1_100_000)),
    )
    .unwrap();
    let oversized = create_test_driver(project_path, runs_dir, true, 1);
    let oversized_outcome = oversized
        .run(&executor, Arc::new(|_| Ok(())), None, None)
        .await;
    assert!(
        matches!(&oversized_outcome, ConvergenceOutcome::Failed { safe_details: Some(details), .. } if details.contains("PC_FROZEN_PLAN_OVERSIZED")),
        "oversized snapshot should fail closed before dispatch, got {oversized_outcome:?}"
    );
    assert_eq!(
        dispatched.load(Ordering::SeqCst),
        0,
        "oversized frozen rendering must fail before dispatch"
    );
}

// ===========================================================================
// Test 9: ESCALATE Decision Routes to WaitingForUser
// ===========================================================================

#[tokio::test]
async fn test_escalate_decision_routes_to_waiting_for_user() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path.clone(), runs_dir, true, 3);

    let p_json = sample_append_proposal_json("V0.23.0-r1.md");
    let executor = MockAdapterExecutor {
        planner_fn: Box::new(move |_, _| Ok(p_json.clone())),
        reviewer_fn: Box::new(move |_, prompt| {
            let (seq, c_id, op_dig, base_dig, ctx_dig) = parse_fields_from_reviewer_prompt(prompt);
            let base_str = match base_dig {
                Some(ref b) => format!("\"basePlanDigest\": \"{b}\","),
                None => String::new(),
            };
            Ok(format!(
                r#"{{
  "schemaVersion": 1,
  "decision": "ESCALATE",
  "reviewedCandidate": {{
    "sequence": {seq},
    "candidateId": "{c_id}",
    "operationPayloadDigest": "{op_dig}",
    {base_str}
    "planContextDigest": "{ctx_dig}"
  }},
  "summary": "Fundamental architectural contradiction",
  "findings": [
    {{
      "findingId": "F-001",
      "affectedRequirement": "Architecture",
      "problem": "Unresolvable requirement collision",
      "whyBlocking": "Requires human decision",
      "requiredChange": "Clarify design intent",
      "scopeEffect": "NONE",
      "blocking": true
    }}
  ]
}}"#
            ))
        }),
        planner_calls: Arc::new(AtomicUsize::new(0)),
        reviewer_calls: Arc::new(AtomicUsize::new(0)),
    };

    let audit_cb: AuditPersistenceFn = Arc::new(|_| Ok(()));
    let outcome = driver.run(&executor, audit_cb, None, None).await;

    match outcome {
        ConvergenceOutcome::WaitingForUser {
            reason,
            diagnostic_code,
            ..
        } => {
            assert_eq!(reason, ConvergenceWaitingReason::ReviewerEscalated);
            assert_eq!(diagnostic_code.as_deref(), Some("PC_REVIEWER_ESCALATED"));
        }
        other => panic!("expected WaitingForUser with ReviewerEscalated, got {other:?}"),
    }
}

// ===========================================================================
// Test 10: Exact Failure Routing (Malformed Output -> INVALID_MODEL_RESPONSE)
// ===========================================================================

#[tokio::test]
async fn test_malformed_model_output_routes_to_invalid_model_response() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path.clone(), runs_dir, true, 3);
    let plan_path = project_path.join(".plan/V0.23.0-r1.md");
    let before = fs::read(&plan_path).unwrap();
    let duplicate_key_output = sample_append_proposal_json("V0.23.0-r1.md").replace(
        "\"schemaVersion\": 1",
        "\"schemaVersion\": 1, \"schema\\u0056ersion\": 1",
    );

    // Planner returns a duplicate key that decodes to the canonical key.
    let executor = MockAdapterExecutor {
        planner_fn: Box::new(move |_, _| Ok(duplicate_key_output.clone())),
        reviewer_fn: Box::new(|_, _| Ok(String::new())),
        planner_calls: Arc::new(AtomicUsize::new(0)),
        reviewer_calls: Arc::new(AtomicUsize::new(0)),
    };

    let planner_audit_events = Arc::new(Mutex::new(Vec::new()));
    let planner_audit_copy = Arc::clone(&planner_audit_events);
    let audit_cb: AuditPersistenceFn = Arc::new(move |entry| {
        if entry.event_type != "dispatch_reserved"
            && entry.event_type != "dispatch_failed"
            && entry.event_type != "dispatch_completed"
        {
            planner_audit_copy.lock().unwrap().push(entry.event_type);
        }
        Ok(())
    });
    let outcome = driver.run(&executor, audit_cb, None, None).await;

    match outcome {
        ConvergenceOutcome::WaitingForUser {
            reason,
            diagnostic_code,
            ..
        } => {
            assert_eq!(reason, ConvergenceWaitingReason::InvalidModelResponse);
            assert_eq!(diagnostic_code.as_deref(), Some("PC_INVALID_PROPOSAL"));
        }
        other => panic!("expected WaitingForUser with InvalidModelResponse, got {other:?}"),
    }
    assert_eq!(executor.planner_calls.load(Ordering::SeqCst), 1);
    assert_eq!(executor.reviewer_calls.load(Ordering::SeqCst), 0);
    assert_eq!(fs::read(&plan_path).unwrap(), before);
    let audit_events = planner_audit_events.lock().unwrap();
    assert_eq!(
        audit_events.as_slice(),
        ["frozen_plan_snapshot_recorded", "planner_proposal_invalid"]
    );
    let outcome = driver
        .run(
            &executor,
            Arc::new(|_| Err("disk unavailable".into())),
            None,
            None,
        )
        .await;
    assert!(
        matches!(outcome, ConvergenceOutcome::Failed { stable_error_code, .. }
        if stable_error_code == "PC_PERSISTENCE_FAILED")
    );
    assert_eq!(executor.reviewer_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_noncanonical_reviewer_output_stops_without_next_dispatch_or_plan_write() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path.clone(), runs_dir, true, 3);
    let plan_path = project_path.join(".plan/V0.23.0-r1.md");
    let before = fs::read(&plan_path).unwrap();
    let planner_calls = Arc::new(AtomicUsize::new(0));
    let reviewer_calls = Arc::new(AtomicUsize::new(0));
    let executor = MockAdapterExecutor {
        planner_fn: Box::new(|_, _| Ok(sample_append_proposal_json("V0.23.0-r1.md"))),
        reviewer_fn: Box::new(move |_, prompt| {
            let (seq, candidate, op, base, context) = parse_fields_from_reviewer_prompt(prompt);
            let canonical =
                sample_approve_verdict_json(seq, &candidate, &op, base.as_deref(), &context);
            Ok(canonical.replace("\"schemaVersion\": 1", "\"schema_version\": 1"))
        }),
        planner_calls: Arc::clone(&planner_calls),
        reviewer_calls: Arc::clone(&reviewer_calls),
    };

    let reviewer_audit_events = Arc::new(Mutex::new(Vec::new()));
    let reviewer_audit_copy = Arc::clone(&reviewer_audit_events);
    let reviewer_audit: AuditPersistenceFn = Arc::new(move |entry| {
        reviewer_audit_copy.lock().unwrap().push(entry.event_type);
        Ok(())
    });
    let outcome = driver.run(&executor, reviewer_audit, None, None).await;
    assert!(matches!(outcome, ConvergenceOutcome::WaitingForUser {
        reason: ConvergenceWaitingReason::InvalidModelResponse,
        diagnostic_code: Some(ref code),
        ..
    } if code == "PC_INVALID_VERDICT"));
    assert_eq!(executor.planner_calls.load(Ordering::SeqCst), 1);
    assert_eq!(executor.reviewer_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fs::read(&plan_path).unwrap(), before);
    let events = reviewer_audit_events.lock().unwrap();
    assert!(events.iter().any(|event| event == "candidate_created"));
    assert!(events.iter().any(|event| event == "verdict_invalid"));
    assert!(!events.iter().any(|event| event == "verdict_recorded"));
}

#[tokio::test]
async fn test_convergence_disk_restart_never_reuses_reserved_review_slots() {
    use crate::orchestrator::recovery::*;
    use crate::orchestrator::types::{
        LoopIterationLimits, RunConfigurationSnapshot, WorkflowState,
    };
    for crash_before_dispatch in [true, false] {
        let (_temp, project_path, runs_dir) = setup_test_workspace();
        let driver = create_test_driver(project_path.clone(), runs_dir.clone(), true, 1);
        let manager = Arc::new(JournalManager::new(runs_dir.clone()));
        let journal = RunJournal {
            schema_version: JOURNAL_SCHEMA_VERSION,
            run_id: driver.run_id.clone(),
            workflow_type: "plan_convergence".into(),
            canonical_project_path: project_path.to_string_lossy().into(),
            task_prompt: Some(driver.task_prompt.clone()),
            approved_plan: None,
            snapshot: RunConfigurationSnapshot {
                project_path: project_path.to_string_lossy().into(),
                assignments: HashMap::new(),
                iteration_limits: LoopIterationLimits {
                    max_plan_review_iterations: 1,
                    ..Default::default()
                },
                validation_gates: vec![],
                budget_limits: HashMap::new(),
                created_at_unix: 1,
                lean_antigravity_mode: false,
                plan_workspace: driver.plan_workspace_config.clone(),
                mcp_servers: HashMap::new(),
            },
            current_state: WorkflowState::PlanDraft,
            last_successful_state: None,
            stage_entry_info: None,
            iteration_counters: RunIterationCounters::default(),
            direct_mcp_invocations: vec![],
            plan_convergence_audit: vec![],
            plan_convergence_confirmation_intent: None,
            usage: None,
            next_dispatch_ordinal: 0,
            adapter_dispatches: vec![],
            revision: 1,
            resume_generation: 0,
            checkpoint_manifest_ref: None,
            checkpoint_digest: None,
            last_shelve_backup_id: None,
            last_shelve_backup_digest: None,
            status: RunRecoveryStatus::Active,
            created_at_unix: 1,
            updated_at_unix: 1,
        };
        manager.write_journal(&journal).unwrap();
        let tracker = Arc::new(Mutex::new(journal));
        let tracker_cb = tracker.clone();
        let manager_cb = manager.clone();
        let audit: AuditPersistenceFn = Arc::new(move |entry| {
            let reserved = entry.event_type == "review_attempt_reserved";
            let mut journal = tracker_cb.lock().unwrap();
            journal.revision += 1;
            if let Some(ref record) = entry.dispatch_record {
                journal.record_adapter_dispatch(record.clone())?;
            }
            if entry.event_type != "dispatch_reserved"
                && entry.event_type != "dispatch_failed"
                && entry.event_type != "dispatch_completed"
            {
                journal.plan_convergence_audit.push(entry);
                journal.iteration_counters.plan_review_count =
                    replay_convergence_review_count(&journal)?;
            }
            manager_cb.write_journal(&journal)?;
            // Simulate process loss after durable reservation, before adapter entry.
            if reserved && crash_before_dispatch {
                return Err("crash after reservation".into());
            }
            Ok(())
        });
        let proposal = sample_append_proposal_json("V0.23.0-r1.md");
        let executor = MockAdapterExecutor {
            planner_fn: Box::new(move |_, _| Ok(proposal.clone())),
            reviewer_fn: Box::new(|_, _| Err("crash after dispatch before verdict".into())),
            planner_calls: Arc::new(AtomicUsize::new(0)),
            reviewer_calls: Arc::new(AtomicUsize::new(0)),
        };
        assert!(matches!(
            driver.run(&executor, audit, None, None).await,
            ConvergenceOutcome::Failed { .. }
        ));
        assert_eq!(
            executor.reviewer_calls.load(Ordering::SeqCst),
            u32::from(!crash_before_dispatch) as usize
        );
        // Construct a fresh manager, reading bytes from disk, not the old tracker.
        let restarted = JournalManager::new(runs_dir.clone());
        let saved = restarted.read_journal(&driver.run_id).unwrap();
        assert_eq!(replay_convergence_review_count(&saved).unwrap(), 1);
        assert_eq!(saved.iteration_counters.plan_review_count, 1);
        let summary = restarted.list_journals().unwrap();
        assert!(!summary[0].is_resumable);
        assert_eq!(summary[0].status, RunRecoveryStatus::Interrupted);
        let calls = (
            executor.planner_calls.load(Ordering::SeqCst),
            executor.reviewer_calls.load(Ordering::SeqCst),
        );
        assert!(matches!(
            driver
                .run(&executor, Arc::new(|_| Ok(())), None, None)
                .await,
            ConvergenceOutcome::WaitingForUser {
                reason: ConvergenceWaitingReason::ReviewLimitReached,
                ..
            }
        ));
        assert_eq!(
            calls,
            (
                executor.planner_calls.load(Ordering::SeqCst),
                executor.reviewer_calls.load(Ordering::SeqCst)
            )
        );
        let mut malformed = saved.clone();
        malformed
            .plan_convergence_audit
            .push(saved.plan_convergence_audit.last().unwrap().clone());
        assert!(replay_convergence_review_count(&malformed).is_err());
        malformed = saved.clone();
        malformed.plan_convergence_audit.reverse();
        assert!(replay_convergence_review_count(&malformed).is_err());
        malformed = saved;
        malformed
            .snapshot
            .iteration_limits
            .max_plan_review_iterations = 0;
        assert!(replay_convergence_review_count(&malformed).is_err());
    }
}

// ===========================================================================
// Test 11: Audit Persistence Failure Injection
// ===========================================================================

#[tokio::test]
async fn test_audit_persistence_failure_injection() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path.clone(), runs_dir, true, 3);

    let p_json = sample_append_proposal_json("V0.23.0-r1.md");
    let executor = MockAdapterExecutor {
        planner_fn: Box::new(move |_, _| Ok(p_json.clone())),
        reviewer_fn: Box::new(move |_, prompt| {
            let (seq, c_id, op_dig, base_dig, ctx_dig) = parse_fields_from_reviewer_prompt(prompt);
            Ok(sample_approve_verdict_json(
                seq,
                &c_id,
                &op_dig,
                base_dig.as_deref(),
                &ctx_dig,
            ))
        }),
        planner_calls: Arc::new(AtomicUsize::new(0)),
        reviewer_calls: Arc::new(AtomicUsize::new(0)),
    };

    // 1. Fail immediately on candidate persistence
    let fail_audit_cb: AuditPersistenceFn = Arc::new(|entry| {
        if entry.event_type == "candidate_created" {
            Err("Disk write failure: candidate".to_string())
        } else {
            Ok(())
        }
    });

    let outcome = driver.run(&executor, fail_audit_cb, None, None).await;
    match outcome {
        ConvergenceOutcome::Failed {
            stable_error_code,
            safe_details,
            ..
        } => {
            assert_eq!(stable_error_code, "PC_PERSISTENCE_FAILED");
            assert!(safe_details
                .unwrap()
                .contains("Disk write failure: candidate"));
        }
        other => panic!("expected Failed, got {other:?}"),
    }

    // 2. Fail on review attempt reservation
    let fail_reservation_cb: AuditPersistenceFn = Arc::new(|entry| {
        if entry.event_type == "review_attempt_reserved" {
            Err("Disk write failure: reservation".to_string())
        } else {
            Ok(())
        }
    });

    let outcome2 = driver.run(&executor, fail_reservation_cb, None, None).await;
    match outcome2 {
        ConvergenceOutcome::Failed {
            stable_error_code,
            safe_details,
            ..
        } => {
            assert_eq!(stable_error_code, "PC_PERSISTENCE_FAILED");
            let details = safe_details.unwrap();
            assert!(
                details.contains("reservation"),
                "unexpected persistence diagnostic: {details}"
            );
        }
        other => panic!("expected Failed, got {other:?}"),
    }

    // 3. Fail on acceptance persistence
    let fail_acceptance_cb: AuditPersistenceFn = Arc::new(|entry| {
        if entry.event_type == "acceptance_succeeded" {
            Err("Disk write failure: acceptance".to_string())
        } else {
            Ok(())
        }
    });

    let outcome3 = driver.run(&executor, fail_acceptance_cb, None, None).await;
    match outcome3 {
        ConvergenceOutcome::Failed {
            stable_error_code,
            safe_details,
            ..
        } => {
            assert_eq!(stable_error_code, "PC_PERSISTENCE_FAILED");
            assert!(safe_details
                .unwrap()
                .contains("Disk write failure: acceptance"));
        }
        other => panic!("expected Failed, got {other:?}"),
    }
}

// ===========================================================================
// Test 12: Review Attempt Reservation Before Reviewer Dispatch
// ===========================================================================

#[tokio::test]
async fn test_review_attempt_reserved_before_reviewer_dispatch() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path.clone(), runs_dir, true, 3);

    let p_json = sample_append_proposal_json("V0.23.0-r1.md");
    let audit_events = Arc::new(Mutex::new(Vec::new()));
    let audit_events_clone = Arc::clone(&audit_events);

    let audit_cb: AuditPersistenceFn = Arc::new(move |entry| {
        if entry.event_type != "dispatch_reserved"
            && entry.event_type != "dispatch_failed"
            && entry.event_type != "dispatch_completed"
        {
            audit_events_clone
                .lock()
                .unwrap()
                .push(entry.event_type.clone());
        }
        Ok(())
    });

    // Reviewer adapter fails with network error
    let audit_events_for_reviewer = Arc::clone(&audit_events);
    let executor = MockAdapterExecutor {
        planner_fn: Box::new(move |_, _| Ok(p_json.clone())),
        reviewer_fn: Box::new(move |_, _| {
            // Verify that review_attempt_reserved was already recorded before this function was invoked
            let current_events = audit_events_for_reviewer.lock().unwrap().clone();
            assert!(
                current_events.contains(&"review_attempt_reserved".to_string()),
                "review_attempt_reserved must be persisted BEFORE reviewer adapter is executed"
            );
            Err("Network unreachable".to_string())
        }),
        planner_calls: Arc::new(AtomicUsize::new(0)),
        reviewer_calls: Arc::new(AtomicUsize::new(0)),
    };

    let outcome = driver.run(&executor, audit_cb, None, None).await;
    match outcome {
        ConvergenceOutcome::Failed {
            stable_error_code,
            safe_details,
            ..
        } => {
            assert_eq!(stable_error_code, "PC_ADAPTER_EXECUTION_FAILED");
            assert!(safe_details.unwrap().contains("Network unreachable"));
        }
        other => panic!("expected Failed with adapter error, got {other:?}"),
    }

    // Ensure audit trail contains review_attempt_reserved
    let events = audit_events.lock().unwrap().clone();
    assert_eq!(
        events,
        vec![
            "frozen_plan_snapshot_recorded",
            "candidate_created",
            "review_attempt_reserved"
        ]
    );
}

// ===========================================================================
// Test 13: Atomic PlanContext Freshness and Append Boundary
// ===========================================================================

#[test]
fn test_plan_append_with_context_validation_atomic_freshness() {
    let (_temp, project_path, _runs_dir) = setup_test_workspace();
    let plan_cfg = PlanWorkspaceConfig::default();

    // 1. Resolve initial context
    let initial_context = resolve_plan_context(&project_path, &plan_cfg).expect("initial context");
    let initial_digest = initial_context.effective_plan_digest.clone().unwrap();

    // 2. Create a supplemental plan file on disk behind our back
    let supp_content = "# Supplemental Plan\n\nAdditional scope.\n";
    fs::write(
        project_path.join(".plan").join("V0.23.0-r1a.md"),
        supp_content,
    )
    .expect("write supplemental plan");

    // 3. Attempt to append with the stale context digest
    let append_req = PlanAppendRequest {
        target_plan_id: "V0.23.0-r1.md".to_string(),
        expected_file_digest: initial_context
            .current_primary_plan
            .as_ref()
            .map(|p| p.digest.clone())
            .unwrap(),
        section_type: "implementation_notes".to_string(),
        section_title: "Race Test".to_string(),
        section_content: "Should not be written.".to_string(),
        idempotency_token: "token-race-1".to_string(),
        idempotency_operation_digest: None,
    };

    let err = plan_append_with_context_validation(
        &project_path,
        &plan_cfg,
        append_req.clone(),
        &initial_digest,
        "V0.23.0-r1.md",
    )
    .unwrap_err();

    assert_eq!(err.code, "stale_plan_context");

    // Verify original plan file was NOT modified
    let primary_bytes =
        fs::read_to_string(project_path.join(".plan").join("V0.23.0-r1.md")).unwrap();
    assert!(!primary_bytes.contains("Race Test"));

    // 4. Resolve fresh context and append successfully to the new leaf
    let fresh_context = resolve_plan_context(&project_path, &plan_cfg).expect("fresh context");
    let fresh_digest = fresh_context.effective_plan_digest.clone().unwrap();
    let target_id = fresh_context.current_leaf_plan_id.clone().unwrap();
    let target_digest = fresh_context
        .active_supplemental_plans
        .iter()
        .find(|s| s.id == target_id)
        .unwrap()
        .digest
        .clone();

    let append_req2 = PlanAppendRequest {
        target_plan_id: target_id.clone(),
        expected_file_digest: target_digest,
        section_type: "implementation_notes".to_string(),
        section_title: "Race Test".to_string(),
        section_content: "Appended cleanly to leaf.".to_string(),
        idempotency_token: "token-race-2".to_string(),
        idempotency_operation_digest: None,
    };

    let resp = plan_append_with_context_validation(
        &project_path,
        &plan_cfg,
        append_req2,
        &fresh_digest,
        &target_id,
    )
    .expect("append with fresh context");

    assert_eq!(resp.plan_id, "V0.23.0-r1a");
    let updated_bytes =
        fs::read_to_string(project_path.join(".plan").join("V0.23.0-r1a.md")).unwrap();
    assert!(updated_bytes.contains("## Race Test"));
    assert!(updated_bytes.contains("Appended cleanly to leaf."));
}

// ===========================================================================
// Test: Live convergence progress emission
// ===========================================================================

fn collect_progress() -> (
    ConvergenceProgressCallback,
    Arc<Mutex<Vec<ConvergenceProgressEvent>>>,
) {
    let events: Arc<Mutex<Vec<ConvergenceProgressEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);
    let callback: ConvergenceProgressCallback = Arc::new(move |event| {
        events_clone.lock().unwrap().push(event);
        Ok(())
    });
    (callback, events)
}

#[tokio::test]
async fn test_progress_events_single_pass_approval_order() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path, runs_dir, true, 3);

    let p_json = sample_append_proposal_json("V0.23.0-r1.md");
    let executor = MockAdapterExecutor {
        planner_fn: Box::new(move |_, _| Ok(p_json.clone())),
        reviewer_fn: Box::new(move |_, prompt| {
            let (seq, c_id, op_dig, base_dig, ctx_dig) = parse_fields_from_reviewer_prompt(prompt);
            Ok(sample_approve_verdict_json(
                seq,
                &c_id,
                &op_dig,
                base_dig.as_deref(),
                &ctx_dig,
            ))
        }),
        planner_calls: Arc::new(AtomicUsize::new(0)),
        reviewer_calls: Arc::new(AtomicUsize::new(0)),
    };

    let (progress_cb, events) = collect_progress();
    let outcome = driver
        .run(&executor, Arc::new(|_| Ok(())), Some(progress_cb), None)
        .await;
    assert!(matches!(outcome, ConvergenceOutcome::Accepted { .. }));

    let recorded = events.lock().unwrap().clone();
    assert_eq!(
        recorded,
        vec![
            ConvergenceProgressEvent::PlannerDispatch { sequence: 1 },
            ConvergenceProgressEvent::ReviewerReserve { sequence: 1 },
            ConvergenceProgressEvent::ReviewerDispatch { sequence: 1 },
            ConvergenceProgressEvent::Verdict {
                sequence: 1,
                decision: ReviewDecision::Approve,
                reviews_used: 1,
                reviews_limit: 3,
            },
        ]
    );
}

#[tokio::test]
async fn test_progress_events_multi_round_then_approval_counters() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path, runs_dir, true, 3);

    let p_json = sample_append_proposal_json("V0.23.0-r1.md");
    let reviewer_calls = Arc::new(AtomicUsize::new(0));
    let reviewer_calls_clone = Arc::clone(&reviewer_calls);
    let executor = MockAdapterExecutor {
        planner_fn: Box::new(move |_, _| Ok(p_json.clone())),
        reviewer_fn: Box::new(move |_, prompt| {
            let (seq, c_id, op_dig, base_dig, ctx_dig) = parse_fields_from_reviewer_prompt(prompt);
            // Request changes twice, then approve the third revision.
            if reviewer_calls_clone.fetch_add(1, Ordering::SeqCst) < 2 {
                Ok(sample_request_changes_verdict_json(
                    seq,
                    &c_id,
                    &op_dig,
                    base_dig.as_deref(),
                    &ctx_dig,
                ))
            } else {
                Ok(sample_approve_verdict_json(
                    seq,
                    &c_id,
                    &op_dig,
                    base_dig.as_deref(),
                    &ctx_dig,
                ))
            }
        }),
        planner_calls: Arc::new(AtomicUsize::new(0)),
        reviewer_calls: Arc::new(AtomicUsize::new(0)),
    };

    let (progress_cb, events) = collect_progress();
    let outcome = driver
        .run(&executor, Arc::new(|_| Ok(())), Some(progress_cb), None)
        .await;

    let recorded = events.lock().unwrap().clone();
    let phases: Vec<&str> = recorded
        .iter()
        .map(|event| match event {
            ConvergenceProgressEvent::PlannerDispatch { .. } => "planner_dispatch",
            ConvergenceProgressEvent::ReviewerReserve { .. } => "reviewer_reserve",
            ConvergenceProgressEvent::ReviewerDispatch { .. } => "reviewer_dispatch",
            ConvergenceProgressEvent::Verdict { .. } => "verdict",
        })
        .collect();
    assert_eq!(
        phases,
        vec![
            "planner_dispatch",
            "reviewer_reserve",
            "reviewer_dispatch",
            "verdict",
            "planner_dispatch",
            "reviewer_reserve",
            "reviewer_dispatch",
            "verdict",
            "planner_dispatch",
            "reviewer_reserve",
            "reviewer_dispatch",
            "verdict",
        ]
    );

    // Only the third verdict approves, and the counters reflect the budget.
    assert!(matches!(
        outcome,
        ConvergenceOutcome::Accepted {
            reviews_used: 3,
            ..
        }
    ));
    let verdicts: Vec<(u32, ReviewDecision, u32, u32)> = recorded
        .iter()
        .filter_map(|event| match event {
            ConvergenceProgressEvent::Verdict {
                sequence,
                decision,
                reviews_used,
                reviews_limit,
            } => Some((*sequence, *decision, *reviews_used, *reviews_limit)),
            _ => None,
        })
        .collect();
    assert_eq!(
        verdicts,
        vec![
            (1, ReviewDecision::RequestChanges, 1, 3),
            (2, ReviewDecision::RequestChanges, 2, 3),
            (3, ReviewDecision::Approve, 3, 3),
        ]
    );
}

#[tokio::test]
async fn test_progress_emission_failure_stops_the_loop() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path, runs_dir, true, 3);

    let p_json = sample_append_proposal_json("V0.23.0-r1.md");
    let planner_calls = Arc::new(AtomicUsize::new(0));
    let planner_calls_clone = Arc::clone(&planner_calls);
    let executor = MockAdapterExecutor {
        planner_fn: Box::new(move |_, _| {
            planner_calls_clone.fetch_add(1, Ordering::SeqCst);
            Ok(p_json.clone())
        }),
        reviewer_fn: Box::new(move |_, prompt| {
            let (seq, c_id, op_dig, base_dig, ctx_dig) = parse_fields_from_reviewer_prompt(prompt);
            Ok(sample_approve_verdict_json(
                seq,
                &c_id,
                &op_dig,
                base_dig.as_deref(),
                &ctx_dig,
            ))
        }),
        planner_calls: Arc::clone(&planner_calls),
        reviewer_calls: Arc::new(AtomicUsize::new(0)),
    };

    let failing_cb: ConvergenceProgressCallback =
        Arc::new(|_| Err("injected event channel failure".to_string()));
    let outcome = driver
        .run(&executor, Arc::new(|_| Ok(())), Some(failing_cb), None)
        .await;

    match outcome {
        ConvergenceOutcome::Failed {
            stable_error_code,
            safe_details,
            ..
        } => {
            assert_eq!(stable_error_code, "PC_PERSISTENCE_FAILED");
            assert!(safe_details
                .unwrap()
                .contains("injected event channel failure"));
        }
        other => panic!("expected Failed, got {other:?}"),
    }
    // The failure happened before the Planner adapter was reached.
    assert_eq!(planner_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_progress_callback_is_optional() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path, runs_dir, true, 3);

    let p_json = sample_append_proposal_json("V0.23.0-r1.md");
    let executor = MockAdapterExecutor {
        planner_fn: Box::new(move |_, _| Ok(p_json.clone())),
        reviewer_fn: Box::new(move |_, prompt| {
            let (seq, c_id, op_dig, base_dig, ctx_dig) = parse_fields_from_reviewer_prompt(prompt);
            Ok(sample_approve_verdict_json(
                seq,
                &c_id,
                &op_dig,
                base_dig.as_deref(),
                &ctx_dig,
            ))
        }),
        planner_calls: Arc::new(AtomicUsize::new(0)),
        reviewer_calls: Arc::new(AtomicUsize::new(0)),
    };

    let outcome = driver
        .run(&executor, Arc::new(|_| Ok(())), None, None)
        .await;
    assert!(matches!(outcome, ConvergenceOutcome::Accepted { .. }));
}

#[tokio::test]
async fn test_usage_and_dispatch_id_propagated_to_audit_and_accepted_outcome() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path, runs_dir, true, 3);
    let p_json = sample_append_proposal_json("V0.23.0-r1.md");

    struct MockUsageExecutor {
        planner_content: String,
    }

    impl ConvergenceAdapterExecutor for MockUsageExecutor {
        async fn execute_role(
            &self,
            _role: AgentRole,
            _profile: &OrchestratorProfile,
            _system_prompt: &str,
            _user_prompt: &str,
            _project_path: &Path,
            _mcp_servers: Option<&HashMap<String, crate::orchestrator::types::McpServerConfig>>,
            _plan_context: Option<&crate::orchestrator::plan_workspace::PlanContext>,
            _frozen_plan: Option<&crate::orchestrator::plan_workspace::FrozenPlanPayload>,
            _timeout: Duration,
            _cancel_token: Option<&CancellationToken>,
        ) -> Result<String, String> {
            unreachable!()
        }

        async fn execute_role_with_output(
            &self,
            role: AgentRole,
            _profile: &OrchestratorProfile,
            _system_prompt: &str,
            user_prompt: &str,
            _project_path: &Path,
            _mcp_servers: Option<&HashMap<String, crate::orchestrator::types::McpServerConfig>>,
            _plan_context: Option<&crate::orchestrator::plan_workspace::PlanContext>,
            _frozen_plan: Option<&crate::orchestrator::plan_workspace::FrozenPlanPayload>,
            _timeout: Duration,
            _cancel_token: Option<&CancellationToken>,
        ) -> Result<RoleExecutionOutput, String> {
            match role {
                AgentRole::Planner => Ok(RoleExecutionOutput {
                    content: self.planner_content.clone(),
                    usage: Some(crate::orchestrator::adapters::NormalizedUsage {
                        input_tokens: Some(100),
                        output_tokens: Some(50),
                        cache_read_tokens: Some(20),
                        cache_write_tokens: Some(0),
                        raw_total_tokens: Some(150),
                    }),
                }),
                AgentRole::PlanReviewer => {
                    let (seq, c_id, op_dig, base_dig, ctx_dig) =
                        parse_fields_from_reviewer_prompt(user_prompt);
                    let content = sample_approve_verdict_json(
                        seq,
                        &c_id,
                        &op_dig,
                        base_dig.as_deref(),
                        &ctx_dig,
                    );
                    Ok(RoleExecutionOutput {
                        content,
                        usage: Some(crate::orchestrator::adapters::NormalizedUsage {
                            input_tokens: Some(200),
                            output_tokens: Some(80),
                            cache_read_tokens: Some(40),
                            cache_write_tokens: Some(10),
                            raw_total_tokens: Some(280),
                        }),
                    })
                }
                _ => Err("Unexpected role".to_string()),
            }
        }
    }

    let audit_entries = Arc::new(Mutex::new(Vec::new()));
    let audit_entries_clone = Arc::clone(&audit_entries);
    let audit_cb: AuditPersistenceFn = Arc::new(move |entry| {
        audit_entries_clone.lock().unwrap().push(entry);
        Ok(())
    });

    let executor = MockUsageExecutor {
        planner_content: p_json,
    };

    let outcome = driver.run(&executor, audit_cb, None, None).await;

    match outcome {
        ConvergenceOutcome::Accepted { usage, .. } => {
            let u = usage.expect("usage must be propagated to Accepted outcome");
            assert_eq!(u.input_tokens, Some(300));
            assert_eq!(u.output_tokens, Some(130));
            assert_eq!(u.cache_read_tokens, Some(60));
            assert_eq!(u.cache_write_tokens, Some(10));
            assert_eq!(u.raw_total_tokens, Some(430));
        }
        other => panic!("expected Accepted, got {other:?}"),
    }

    let entries = audit_entries.lock().unwrap().clone();
    let candidate_entry = entries
        .iter()
        .find(|e| e.event_type == "candidate_created")
        .unwrap();
    assert_eq!(
        candidate_entry.dispatch_id,
        Some(format!("{}:1", driver.run_id))
    );
    assert_eq!(candidate_entry.dispatch_ordinal, Some(1));
    assert_eq!(
        candidate_entry.usage.as_ref().and_then(|u| u.input_tokens),
        Some(100)
    );

    let verdict_entry = entries
        .iter()
        .find(|e| e.event_type == "verdict_recorded")
        .unwrap();
    assert_eq!(
        verdict_entry.dispatch_id,
        Some(format!("{}:2", driver.run_id))
    );
    assert_eq!(verdict_entry.dispatch_ordinal, Some(2));
    assert_eq!(
        verdict_entry.usage.as_ref().and_then(|u| u.input_tokens),
        Some(200)
    );
}

#[tokio::test]
async fn test_planner_and_reviewer_success_persistence_ordering() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path, runs_dir, true, 3);
    let p_json = sample_append_proposal_json("V0.23.0-r1.md");

    struct OrderedExecutor {
        p_json: String,
    }
    impl ConvergenceAdapterExecutor for OrderedExecutor {
        async fn execute_role(
            &self,
            _role: AgentRole,
            _profile: &OrchestratorProfile,
            _system_prompt: &str,
            _user_prompt: &str,
            _project_path: &Path,
            _mcp_servers: Option<&HashMap<String, crate::orchestrator::types::McpServerConfig>>,
            _plan_context: Option<&crate::orchestrator::plan_workspace::PlanContext>,
            _frozen_plan: Option<&crate::orchestrator::plan_workspace::FrozenPlanPayload>,
            _timeout: Duration,
            _cancel_token: Option<&CancellationToken>,
        ) -> Result<String, String> {
            unreachable!()
        }

        async fn execute_role_with_output(
            &self,
            role: AgentRole,
            _profile: &OrchestratorProfile,
            _system_prompt: &str,
            user_prompt: &str,
            _project_path: &Path,
            _mcp_servers: Option<&HashMap<String, crate::orchestrator::types::McpServerConfig>>,
            _plan_context: Option<&crate::orchestrator::plan_workspace::PlanContext>,
            _frozen_plan: Option<&crate::orchestrator::plan_workspace::FrozenPlanPayload>,
            _timeout: Duration,
            _cancel_token: Option<&CancellationToken>,
        ) -> Result<RoleExecutionOutput, String> {
            match role {
                AgentRole::Planner => Ok(RoleExecutionOutput {
                    content: self.p_json.clone(),
                    usage: Some(crate::orchestrator::adapters::NormalizedUsage {
                        input_tokens: Some(50),
                        output_tokens: Some(25),
                        ..Default::default()
                    }),
                }),
                AgentRole::PlanReviewer => {
                    let (seq, c_id, op_dig, base_dig, ctx_dig) =
                        parse_fields_from_reviewer_prompt(user_prompt);
                    Ok(RoleExecutionOutput {
                        content: sample_approve_verdict_json(
                            seq,
                            &c_id,
                            &op_dig,
                            base_dig.as_deref(),
                            &ctx_dig,
                        ),
                        usage: Some(crate::orchestrator::adapters::NormalizedUsage {
                            input_tokens: Some(60),
                            output_tokens: Some(30),
                            ..Default::default()
                        }),
                    })
                }
                _ => Err("unexpected role".into()),
            }
        }
    }

    let event_order = Arc::new(Mutex::new(Vec::new()));
    let order_clone = Arc::clone(&event_order);
    let audit_cb: AuditPersistenceFn = Arc::new(move |entry| {
        order_clone.lock().unwrap().push((
            entry.event_type.clone(),
            entry.role.clone(),
            entry.dispatch_record.is_some(),
        ));
        Ok(())
    });

    let executor = OrderedExecutor { p_json };
    let outcome = driver.run(&executor, audit_cb, None, None).await;
    assert!(matches!(outcome, ConvergenceOutcome::Accepted { .. }));

    let order = event_order.lock().unwrap().clone();
    let planner_reserved_idx = order
        .iter()
        .position(|(e, r, _)| e == "dispatch_reserved" && r.as_deref() == Some("planner"))
        .unwrap();
    let planner_completed_idx = order
        .iter()
        .position(|(e, r, d)| e == "dispatch_completed" && r.as_deref() == Some("planner") && *d)
        .unwrap();
    let candidate_created_idx = order
        .iter()
        .position(|(e, ..)| e == "candidate_created")
        .unwrap();
    assert!(
        planner_reserved_idx < planner_completed_idx,
        "dispatch_reserved must precede dispatch_completed"
    );
    assert!(
        planner_completed_idx < candidate_created_idx,
        "dispatch_completed must precede candidate_created"
    );

    let reviewer_reserved_idx = order
        .iter()
        .position(|(e, ..)| e == "review_attempt_reserved")
        .unwrap();
    let reviewer_completed_idx = order
        .iter()
        .position(|(e, r, d)| {
            e == "dispatch_completed" && r.as_deref() == Some("plan_reviewer") && *d
        })
        .unwrap();
    let verdict_recorded_idx = order
        .iter()
        .position(|(e, ..)| e == "verdict_recorded")
        .unwrap();
    assert!(
        reviewer_reserved_idx < reviewer_completed_idx,
        "review_attempt_reserved must precede dispatch_completed"
    );
    assert!(
        reviewer_completed_idx < verdict_recorded_idx,
        "dispatch_completed must precede verdict_recorded"
    );
}

#[tokio::test]
async fn test_later_failure_and_cancellation_retains_usage() {
    // Part A: Planner succeeds (usage persisted) but proposal parsing fails.
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path.clone(), runs_dir.clone(), true, 3);

    struct BadProposalExecutor;
    impl ConvergenceAdapterExecutor for BadProposalExecutor {
        async fn execute_role(
            &self,
            _role: AgentRole,
            _profile: &OrchestratorProfile,
            _system_prompt: &str,
            _user_prompt: &str,
            _project_path: &Path,
            _mcp_servers: Option<&HashMap<String, crate::orchestrator::types::McpServerConfig>>,
            _plan_context: Option<&crate::orchestrator::plan_workspace::PlanContext>,
            _frozen_plan: Option<&crate::orchestrator::plan_workspace::FrozenPlanPayload>,
            _timeout: Duration,
            _cancel_token: Option<&CancellationToken>,
        ) -> Result<String, String> {
            unreachable!()
        }
        async fn execute_role_with_output(
            &self,
            _role: AgentRole,
            _profile: &OrchestratorProfile,
            _system_prompt: &str,
            _user_prompt: &str,
            _project_path: &Path,
            _mcp_servers: Option<&HashMap<String, crate::orchestrator::types::McpServerConfig>>,
            _plan_context: Option<&crate::orchestrator::plan_workspace::PlanContext>,
            _frozen_plan: Option<&crate::orchestrator::plan_workspace::FrozenPlanPayload>,
            _timeout: Duration,
            _cancel_token: Option<&CancellationToken>,
        ) -> Result<RoleExecutionOutput, String> {
            Ok(RoleExecutionOutput {
                content: "Not JSON at all".to_string(),
                usage: Some(crate::orchestrator::adapters::NormalizedUsage {
                    input_tokens: Some(150),
                    output_tokens: Some(75),
                    ..Default::default()
                }),
            })
        }
    }

    let manager = Arc::new(crate::orchestrator::recovery::JournalManager::new(
        runs_dir.clone(),
    ));
    let tracker = Arc::new(Mutex::new(crate::orchestrator::recovery::RunJournal {
        schema_version: crate::orchestrator::recovery::JOURNAL_SCHEMA_VERSION,
        run_id: driver.run_id.clone(),
        workflow_type: "plan_convergence".into(),
        canonical_project_path: project_path.to_string_lossy().into(),
        task_prompt: Some(driver.task_prompt.clone()),
        approved_plan: None,
        snapshot: crate::orchestrator::types::RunConfigurationSnapshot {
            project_path: project_path.to_string_lossy().into(),
            assignments: HashMap::new(),
            iteration_limits: crate::orchestrator::types::LoopIterationLimits::default(),
            validation_gates: vec![],
            budget_limits: HashMap::new(),
            created_at_unix: 1,
            lean_antigravity_mode: false,
            plan_workspace: driver.plan_workspace_config.clone(),
            mcp_servers: HashMap::new(),
        },
        current_state: crate::orchestrator::types::WorkflowState::PlanDraft,
        last_successful_state: None,
        stage_entry_info: None,
        iteration_counters: crate::orchestrator::recovery::RunIterationCounters::default(),
        direct_mcp_invocations: vec![],
        plan_convergence_audit: vec![],
        plan_convergence_confirmation_intent: None,
        usage: None,
        next_dispatch_ordinal: 0,
        adapter_dispatches: vec![],
        revision: 1,
        resume_generation: 0,
        checkpoint_manifest_ref: None,
        checkpoint_digest: None,
        last_shelve_backup_id: None,
        last_shelve_backup_digest: None,
        status: crate::orchestrator::recovery::RunRecoveryStatus::Active,
        created_at_unix: 1,
        updated_at_unix: 1,
    }));
    manager.write_journal(&tracker.lock().unwrap()).unwrap();

    let tracker_clone = Arc::clone(&tracker);
    let manager_clone = Arc::clone(&manager);
    let audit_cb: AuditPersistenceFn = Arc::new(move |entry| {
        let mut j = tracker_clone.lock().unwrap();
        j.revision += 1;
        if let Some(ref record) = entry.dispatch_record {
            j.record_adapter_dispatch(record.clone())?;
        }
        if entry.event_type != "dispatch_reserved"
            && entry.event_type != "dispatch_failed"
            && entry.event_type != "dispatch_completed"
        {
            j.plan_convergence_audit.push(entry);
        }
        manager_clone.write_journal(&j)?;
        Ok(())
    });

    let outcome = driver.run(&BadProposalExecutor, audit_cb, None, None).await;
    match outcome {
        ConvergenceOutcome::WaitingForUser { usage, reason, .. } => {
            assert_eq!(reason, ConvergenceWaitingReason::InvalidModelResponse);
            let u =
                usage.expect("WaitingForUser must retain usage from completed planner dispatch");
            assert_eq!(u.input_tokens, Some(150));
            assert_eq!(u.output_tokens, Some(75));
        }
        other => panic!("expected WaitingForUser, got {other:?}"),
    }
    let j = tracker.lock().unwrap();
    assert_eq!(j.adapter_dispatches.len(), 1);
    assert_eq!(j.adapter_dispatches[0].status.as_deref(), Some("completed"));
    assert_eq!(
        j.adapter_dispatches[0]
            .usage
            .as_ref()
            .and_then(|u| u.input_tokens),
        Some(150)
    );

    // Part B: Planner succeeds, then cancellation happens before reviewer finishes.
    let (_temp2, project_path2, runs_dir2) = setup_test_workspace();
    let driver2 = create_test_driver(project_path2.clone(), runs_dir2.clone(), true, 3);
    let p_json = sample_append_proposal_json("V0.23.0-r1.md");

    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();

    struct CancelOnReviewerExecutor {
        p_json: String,
        cancel_token: CancellationToken,
    }
    impl ConvergenceAdapterExecutor for CancelOnReviewerExecutor {
        async fn execute_role(
            &self,
            _role: AgentRole,
            _profile: &OrchestratorProfile,
            _system_prompt: &str,
            _user_prompt: &str,
            _project_path: &Path,
            _mcp_servers: Option<&HashMap<String, crate::orchestrator::types::McpServerConfig>>,
            _plan_context: Option<&crate::orchestrator::plan_workspace::PlanContext>,
            _frozen_plan: Option<&crate::orchestrator::plan_workspace::FrozenPlanPayload>,
            _timeout: Duration,
            _cancel_token: Option<&CancellationToken>,
        ) -> Result<String, String> {
            unreachable!()
        }
        async fn execute_role_with_output(
            &self,
            role: AgentRole,
            _profile: &OrchestratorProfile,
            _system_prompt: &str,
            _user_prompt: &str,
            _project_path: &Path,
            _mcp_servers: Option<&HashMap<String, crate::orchestrator::types::McpServerConfig>>,
            _plan_context: Option<&crate::orchestrator::plan_workspace::PlanContext>,
            _frozen_plan: Option<&crate::orchestrator::plan_workspace::FrozenPlanPayload>,
            _timeout: Duration,
            _cancel_token: Option<&CancellationToken>,
        ) -> Result<RoleExecutionOutput, String> {
            match role {
                AgentRole::Planner => Ok(RoleExecutionOutput {
                    content: self.p_json.clone(),
                    usage: Some(crate::orchestrator::adapters::NormalizedUsage {
                        input_tokens: Some(200),
                        output_tokens: Some(80),
                        ..Default::default()
                    }),
                }),
                AgentRole::PlanReviewer => {
                    self.cancel_token.cancel();
                    Err("cancelled during reviewer".to_string())
                }
                _ => Err("unexpected".into()),
            }
        }
    }

    let tracker2 = Arc::new(Mutex::new(crate::orchestrator::recovery::RunJournal {
        schema_version: crate::orchestrator::recovery::JOURNAL_SCHEMA_VERSION,
        run_id: driver2.run_id.clone(),
        workflow_type: "plan_convergence".into(),
        canonical_project_path: project_path2.to_string_lossy().into(),
        task_prompt: Some(driver2.task_prompt.clone()),
        approved_plan: None,
        snapshot: crate::orchestrator::types::RunConfigurationSnapshot {
            project_path: project_path2.to_string_lossy().into(),
            assignments: HashMap::new(),
            iteration_limits: crate::orchestrator::types::LoopIterationLimits::default(),
            validation_gates: vec![],
            budget_limits: HashMap::new(),
            created_at_unix: 1,
            lean_antigravity_mode: false,
            plan_workspace: driver2.plan_workspace_config.clone(),
            mcp_servers: HashMap::new(),
        },
        current_state: crate::orchestrator::types::WorkflowState::PlanDraft,
        last_successful_state: None,
        stage_entry_info: None,
        iteration_counters: crate::orchestrator::recovery::RunIterationCounters::default(),
        direct_mcp_invocations: vec![],
        plan_convergence_audit: vec![],
        plan_convergence_confirmation_intent: None,
        usage: None,
        next_dispatch_ordinal: 0,
        adapter_dispatches: vec![],
        revision: 1,
        resume_generation: 0,
        checkpoint_manifest_ref: None,
        checkpoint_digest: None,
        last_shelve_backup_id: None,
        last_shelve_backup_digest: None,
        status: crate::orchestrator::recovery::RunRecoveryStatus::Active,
        created_at_unix: 1,
        updated_at_unix: 1,
    }));
    let tracker2_clone = Arc::clone(&tracker2);
    let audit_cb2: AuditPersistenceFn = Arc::new(move |entry| {
        let mut j = tracker2_clone.lock().unwrap();
        if let Some(ref record) = entry.dispatch_record {
            j.record_adapter_dispatch(record.clone())?;
        }
        Ok(())
    });

    let outcome2 = driver2
        .run(
            &CancelOnReviewerExecutor {
                p_json: p_json.clone(),
                cancel_token,
            },
            audit_cb2,
            None,
            Some(&cancel_token_clone),
        )
        .await;
    assert!(matches!(outcome2, ConvergenceOutcome::Cancelled));
    let j2 = tracker2.lock().unwrap();
    let planner_dispatch = j2
        .adapter_dispatches
        .iter()
        .find(|d| d.role == "planner")
        .unwrap();
    assert_eq!(planner_dispatch.status.as_deref(), Some("completed"));
    assert_eq!(
        planner_dispatch.usage.as_ref().and_then(|u| u.input_tokens),
        Some(200)
    );
}

#[tokio::test]
async fn test_failed_dispatch_persistence_error_precedence_and_details() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path, runs_dir, true, 3);
    let p_json = sample_append_proposal_json("V0.23.0-r1.md");

    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();

    struct FailReviewerExecutor {
        p_json: String,
        cancel_token: CancellationToken,
    }
    impl ConvergenceAdapterExecutor for FailReviewerExecutor {
        async fn execute_role(
            &self,
            _role: AgentRole,
            _profile: &OrchestratorProfile,
            _system_prompt: &str,
            _user_prompt: &str,
            _project_path: &Path,
            _mcp_servers: Option<&HashMap<String, crate::orchestrator::types::McpServerConfig>>,
            _plan_context: Option<&crate::orchestrator::plan_workspace::PlanContext>,
            _frozen_plan: Option<&crate::orchestrator::plan_workspace::FrozenPlanPayload>,
            _timeout: Duration,
            _cancel_token: Option<&CancellationToken>,
        ) -> Result<String, String> {
            unreachable!()
        }
        async fn execute_role_with_output(
            &self,
            role: AgentRole,
            _profile: &OrchestratorProfile,
            _system_prompt: &str,
            _user_prompt: &str,
            _project_path: &Path,
            _mcp_servers: Option<&HashMap<String, crate::orchestrator::types::McpServerConfig>>,
            _plan_context: Option<&crate::orchestrator::plan_workspace::PlanContext>,
            _frozen_plan: Option<&crate::orchestrator::plan_workspace::FrozenPlanPayload>,
            _timeout: Duration,
            _cancel_token: Option<&CancellationToken>,
        ) -> Result<RoleExecutionOutput, String> {
            match role {
                AgentRole::Planner => Ok(RoleExecutionOutput {
                    content: self.p_json.clone(),
                    usage: None,
                }),
                AgentRole::PlanReviewer => {
                    self.cancel_token.cancel();
                    Err("upstream connection reset".to_string())
                }
                _ => Err("unexpected".into()),
            }
        }
    }

    let audit_cb: AuditPersistenceFn = Arc::new(|entry| {
        if entry.event_type == "dispatch_failed" {
            Err("injected disk quota exceeded on failed dispatch".to_string())
        } else {
            Ok(())
        }
    });

    let executor = FailReviewerExecutor {
        p_json,
        cancel_token,
    };
    let outcome = driver
        .run(&executor, audit_cb, None, Some(&cancel_token_clone))
        .await;

    match outcome {
        ConvergenceOutcome::Failed {
            stable_error_code,
            safe_details,
            ..
        } => {
            assert_eq!(
                stable_error_code, "PC_PERSISTENCE_FAILED",
                "persistence failure must take precedence over cancellation"
            );
            let details = safe_details.expect("safe_details must be present");
            assert!(
                details.contains("injected disk quota exceeded on failed dispatch"),
                "must retain persistence error"
            );
            assert!(
                details.contains("upstream connection reset"),
                "must retain original adapter error in safe diagnostic details"
            );
        }
        other => panic!("expected PC_PERSISTENCE_FAILED, got {other:?}"),
    }
}

struct CustomMockExecutor {
    planner_res: Result<RoleExecutionOutput, String>,
    reviewer_res: Result<RoleExecutionOutput, String>,
}

impl ConvergenceAdapterExecutor for CustomMockExecutor {
    async fn execute_role(
        &self,
        _role: AgentRole,
        _profile: &OrchestratorProfile,
        _system_prompt: &str,
        _user_prompt: &str,
        _project_path: &Path,
        _mcp_servers: Option<&HashMap<String, crate::orchestrator::types::McpServerConfig>>,
        _plan_context: Option<&crate::orchestrator::plan_workspace::PlanContext>,
        _frozen_plan: Option<&crate::orchestrator::plan_workspace::FrozenPlanPayload>,
        _timeout: Duration,
        _cancel_token: Option<&CancellationToken>,
    ) -> Result<String, String> {
        unreachable!()
    }

    async fn execute_role_with_output(
        &self,
        role: AgentRole,
        _profile: &OrchestratorProfile,
        _system_prompt: &str,
        _user_prompt: &str,
        _project_path: &Path,
        _mcp_servers: Option<&HashMap<String, crate::orchestrator::types::McpServerConfig>>,
        _plan_context: Option<&crate::orchestrator::plan_workspace::PlanContext>,
        _frozen_plan: Option<&crate::orchestrator::plan_workspace::FrozenPlanPayload>,
        _timeout: Duration,
        _cancel_token: Option<&CancellationToken>,
    ) -> Result<RoleExecutionOutput, String> {
        match role {
            AgentRole::Planner => self.planner_res.clone(),
            AgentRole::PlanReviewer => self.reviewer_res.clone(),
            _ => Err("unexpected role".into()),
        }
    }
}

/// Helper to seed the on-disk journal with persisted adapter dispatches.
/// Lifecycle events like `dispatch_reserved` populate `adapter_dispatches`
/// while leaving `plan_convergence_audit` empty, exactly matching the shape
/// produced by `apply_audit_entry_transaction`.
fn setup_journal_with_dispatches(
    project_path: &Path,
    runs_dir: &Path,
    driver: &ConvergenceDriver,
    dispatches: Vec<crate::orchestrator::recovery::AdapterDispatchRecord>,
) -> Arc<crate::orchestrator::recovery::JournalManager> {
    let manager = Arc::new(crate::orchestrator::recovery::JournalManager::new(
        runs_dir.to_path_buf(),
    ));
    let next_dispatch_ordinal = dispatches
        .iter()
        .map(|d| d.ordinal)
        .max()
        .map(|m| m.saturating_add(1))
        .unwrap_or(1);
    let journal = crate::orchestrator::recovery::RunJournal {
        schema_version: crate::orchestrator::recovery::JOURNAL_SCHEMA_VERSION,
        run_id: driver.run_id.clone(),
        workflow_type: "plan_convergence".into(),
        canonical_project_path: project_path.to_string_lossy().into(),
        task_prompt: Some(driver.task_prompt.clone()),
        approved_plan: None,
        snapshot: crate::orchestrator::types::RunConfigurationSnapshot {
            project_path: project_path.to_string_lossy().into(),
            assignments: HashMap::new(),
            iteration_limits: crate::orchestrator::types::LoopIterationLimits::default(),
            validation_gates: vec![],
            budget_limits: HashMap::new(),
            created_at_unix: 1,
            lean_antigravity_mode: false,
            plan_workspace: driver.plan_workspace_config.clone(),
            mcp_servers: HashMap::new(),
        },
        current_state: crate::orchestrator::types::WorkflowState::PlanDraft,
        last_successful_state: None,
        stage_entry_info: None,
        iteration_counters: crate::orchestrator::recovery::RunIterationCounters::default(),
        direct_mcp_invocations: vec![],
        plan_convergence_audit: vec![],
        plan_convergence_confirmation_intent: None,
        usage: None,
        next_dispatch_ordinal,
        adapter_dispatches: dispatches,
        revision: 1,
        resume_generation: 0,
        checkpoint_manifest_ref: None,
        checkpoint_digest: None,
        last_shelve_backup_id: None,
        last_shelve_backup_digest: None,
        status: crate::orchestrator::recovery::RunRecoveryStatus::Active,
        created_at_unix: 1,
        updated_at_unix: 1,
    };
    manager.write_journal(&journal).unwrap();
    manager
}

#[tokio::test]
async fn test_restart_usage_aggregation_all_terminal_known() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path.clone(), runs_dir.clone(), true, 3);
    let d1 = crate::orchestrator::recovery::AdapterDispatchRecord {
        dispatch_id: format!("{}:1", driver.run_id),
        ordinal: 1,
        role: "planner".to_string(),
        profile_id: "test-prof".to_string(),
        model: Some("test-model".to_string()),
        payload_digest: Some("digest1".to_string()),
        status: Some("completed".to_string()),
        usage: Some(crate::orchestrator::adapters::NormalizedUsage {
            input_tokens: Some(100),
            output_tokens: Some(50),
            raw_total_tokens: Some(150),
            cache_read_tokens: Some(10),
            cache_write_tokens: Some(5),
        }),
        created_at_unix: 1,
        completed_at_unix: Some(2),
    };
    let d2 = crate::orchestrator::recovery::AdapterDispatchRecord {
        dispatch_id: format!("{}:2", driver.run_id),
        ordinal: 2,
        role: "plan_reviewer".to_string(),
        profile_id: "test-prof".to_string(),
        model: Some("test-model".to_string()),
        payload_digest: Some("digest2".to_string()),
        status: Some("failed".to_string()),
        usage: Some(crate::orchestrator::adapters::NormalizedUsage {
            input_tokens: Some(200),
            output_tokens: Some(25),
            raw_total_tokens: Some(225),
            cache_read_tokens: Some(20),
            cache_write_tokens: Some(10),
        }),
        created_at_unix: 3,
        completed_at_unix: Some(4),
    };
    let _manager = setup_journal_with_dispatches(&project_path, &runs_dir, &driver, vec![d1, d2]);

    let executor = CustomMockExecutor {
        planner_res: Ok(RoleExecutionOutput {
            content: "invalid json".to_string(),
            usage: Some(crate::orchestrator::adapters::NormalizedUsage {
                input_tokens: Some(0),
                output_tokens: Some(0),
                raw_total_tokens: Some(0),
                cache_read_tokens: Some(0),
                cache_write_tokens: Some(0),
            }),
        }),
        reviewer_res: Err("unused".to_string()),
    };

    let outcome = driver
        .run(&executor, Arc::new(|_| Ok(())), None, None)
        .await;
    match outcome {
        ConvergenceOutcome::WaitingForUser { usage, reason, .. } => {
            assert_eq!(reason, ConvergenceWaitingReason::InvalidModelResponse);
            let u = usage.expect("all terminal dispatches with known usage must aggregate");
            assert_eq!(u.input_tokens, Some(300));
            assert_eq!(u.output_tokens, Some(75));
            assert_eq!(u.raw_total_tokens, Some(375));
            assert_eq!(u.cache_read_tokens, Some(30));
            assert_eq!(u.cache_write_tokens, Some(15));
        }
        other => panic!("expected WaitingForUser, got {other:?}"),
    }
}

#[tokio::test]
async fn test_restart_usage_aggregation_unresolved_or_unknown_yields_unknown() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path.clone(), runs_dir.clone(), true, 3);
    let d1 = crate::orchestrator::recovery::AdapterDispatchRecord {
        dispatch_id: format!("{}:1", driver.run_id),
        ordinal: 1,
        role: "planner".to_string(),
        profile_id: "test-prof".to_string(),
        model: Some("test-model".to_string()),
        payload_digest: Some("digest1".to_string()),
        status: Some("completed".to_string()),
        usage: Some(crate::orchestrator::adapters::NormalizedUsage {
            input_tokens: Some(100),
            output_tokens: Some(50),
            ..Default::default()
        }),
        created_at_unix: 1,
        completed_at_unix: Some(2),
    };
    // d2 is unresolved ("reserved")
    let d2 = crate::orchestrator::recovery::AdapterDispatchRecord {
        dispatch_id: format!("{}:2", driver.run_id),
        ordinal: 2,
        role: "plan_reviewer".to_string(),
        profile_id: "test-prof".to_string(),
        model: Some("test-model".to_string()),
        payload_digest: Some("digest2".to_string()),
        status: Some("reserved".to_string()),
        usage: None,
        created_at_unix: 3,
        completed_at_unix: None,
    };
    // d3 has unknown non-terminal status
    let d3 = crate::orchestrator::recovery::AdapterDispatchRecord {
        dispatch_id: format!("{}:3", driver.run_id),
        ordinal: 3,
        role: "planner".to_string(),
        profile_id: "test-prof".to_string(),
        model: Some("test-model".to_string()),
        payload_digest: Some("digest3".to_string()),
        status: Some("custom_unresolved".to_string()),
        usage: None,
        created_at_unix: 4,
        completed_at_unix: None,
    };
    let _manager =
        setup_journal_with_dispatches(&project_path, &runs_dir, &driver, vec![d1, d2, d3]);

    let executor = CustomMockExecutor {
        planner_res: Ok(RoleExecutionOutput {
            content: "invalid json".to_string(),
            usage: None,
        }),
        reviewer_res: Err("unused".to_string()),
    };

    let outcome = driver
        .run(&executor, Arc::new(|_| Ok(())), None, None)
        .await;
    match outcome {
        ConvergenceOutcome::WaitingForUser { usage, reason, .. } => {
            assert_eq!(reason, ConvergenceWaitingReason::InvalidModelResponse);
            assert!(
                usage.is_none(),
                "presence of unresolved or unknown dispatch states must yield unknown usage"
            );
        }
        other => panic!("expected WaitingForUser, got {other:?}"),
    }
}

#[tokio::test]
async fn test_restart_usage_aggregation_reserved_followed_by_new_completed_remains_unknown() {
    let (_temp, project_path, runs_dir) = setup_test_workspace();
    let driver = create_test_driver(project_path.clone(), runs_dir.clone(), true, 3);
    // Journal has a reserved dispatch before restart
    let d1 = crate::orchestrator::recovery::AdapterDispatchRecord {
        dispatch_id: format!("{}:1", driver.run_id),
        ordinal: 1,
        role: "planner".to_string(),
        profile_id: "test-prof".to_string(),
        model: Some("test-model".to_string()),
        payload_digest: Some("digest1".to_string()),
        status: Some("reserved".to_string()),
        usage: None,
        created_at_unix: 1,
        completed_at_unix: None,
    };
    let _manager = setup_journal_with_dispatches(&project_path, &runs_dir, &driver, vec![d1]);

    let p_json = sample_append_proposal_json("V0.23.0-r1.md");
    // New planner dispatch succeeds with known usage, reviewer then returns invalid JSON
    let executor = CustomMockExecutor {
        planner_res: Ok(RoleExecutionOutput {
            content: p_json,
            usage: Some(crate::orchestrator::adapters::NormalizedUsage {
                input_tokens: Some(500),
                output_tokens: Some(100),
                raw_total_tokens: Some(600),
                cache_read_tokens: Some(50),
                cache_write_tokens: Some(25),
            }),
        }),
        reviewer_res: Ok(RoleExecutionOutput {
            content: "invalid reviewer verdict json".to_string(),
            usage: Some(crate::orchestrator::adapters::NormalizedUsage {
                input_tokens: Some(150),
                output_tokens: Some(30),
                ..Default::default()
            }),
        }),
    };

    let outcome = driver
        .run(&executor, Arc::new(|_| Ok(())), None, None)
        .await;
    match outcome {
        ConvergenceOutcome::WaitingForUser { usage, reason, .. } => {
            assert_eq!(reason, ConvergenceWaitingReason::InvalidModelResponse);
            assert!(
                usage.is_none(),
                "new completed dispatch usage must not overwrite unknown usage caused by prior reserved dispatch"
            );
        }
        other => panic!("expected WaitingForUser, got {other:?}"),
    }
}
