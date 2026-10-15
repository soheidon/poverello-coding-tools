use anthro_bridge_mcp_server::mcp::{build_system_prompt, build_user_prompt};

#[test]
fn system_prompt_sets_planner_role() {
    let prompt = build_system_prompt();
    assert!(prompt.contains("implementation planner"));
    assert!(prompt.contains("not an implementation agent"));
}

#[test]
fn system_prompt_forbids_fabrication() {
    let prompt = build_system_prompt();
    assert!(prompt.contains("Never claim that a change has already been made"));
    assert!(prompt.contains("Never invent or fabricate test results"));
}

#[test]
fn system_prompt_forbids_assuming_repository_access() {
    let prompt = build_system_prompt();
    assert!(prompt.contains("Never assume access to the repository"));
}

#[test]
fn user_prompt_includes_task_and_context() {
    let prompt = build_user_prompt(
        "add a save button",
        "SaveButton.tsx is a React component",
        None,
    );
    assert!(prompt.contains("add a save button"));
    assert!(prompt.contains("SaveButton.tsx"));
    assert!(!prompt.contains("## Constraints"));
}

#[test]
fn user_prompt_includes_constraints_when_present() {
    let prompt = build_user_prompt("task", "context", Some("do not commit"));
    assert!(prompt.contains("do not commit"));
    assert!(prompt.contains("## Constraints"));
}

#[test]
fn user_prompt_omits_blank_constraints() {
    let prompt = build_user_prompt("task", "context", Some("   "));
    assert!(!prompt.contains("## Constraints"));
}

#[test]
fn review_system_prompt_defines_read_only_and_verdicts() {
    use anthro_bridge_mcp_server::mcp::build_review_system_prompt;

    let prompt = build_review_system_prompt();
    assert!(prompt.contains("read-only software implementation reviewer"));
    assert!(prompt.contains("Decision: <Approved | Approved with recommendations | Not approved>"));
    assert!(prompt.contains("Commit readiness: <READY | NOT READY>"));
    assert!(prompt.contains("Tests passing are necessary but not sufficient for approval"));
    assert!(prompt.contains("Scope compliance"));
    assert!(prompt.contains("Plan compliance"));
    assert!(prompt.contains("Contract preservation"));
    assert!(prompt.contains("Determinism & RNG"));
    assert!(prompt.contains("Concurrency & parallelism"));
}

#[test]
fn review_user_prompt_formats_sections_reliably() {
    use anthro_bridge_mcp_server::mcp::{build_review_user_prompt, ReviewParams};

    let params = ReviewParams {
        task: "Fix boundary condition in timer".to_string(),
        approved_plan: "1. Add check\n2. Run tests".to_string(),
        git_diff: "+ if x <= 0 { return; }".to_string(),
        git_status: "M timer.rs".to_string(),
        test_results: Some("3 tests passed".to_string()),
        review_mode: Some("deep".to_string()),
        additional_context: Some("Called by orchestrator.rs".to_string()),
    };

    let prompt = build_review_user_prompt(&params);
    assert!(prompt.contains("## Task Summary\n\nFix boundary condition"));
    assert!(prompt.contains("## Approved Implementation Plan\n\n1. Add check"));
    assert!(prompt.contains("## Working Tree Status (git status)\n\nM timer.rs"));
    assert!(prompt.contains("## Implementation Diff\n\n+ if x <= 0 { return; }"));
    assert!(prompt.contains("## Test Results\n\n3 tests passed"));
    assert!(prompt.contains("## Review Mode\n\ndeep"));
    assert!(prompt.contains("## Additional Repository Context & Contracts\n\nCalled by orchestrator.rs"));
}

#[test]
fn resolve_review_mode_handles_valid_and_invalid_modes() {
    use anthro_bridge_mcp_server::mcp::resolve_review_mode;

    // None -> standard
    assert_eq!(resolve_review_mode(None).unwrap(), "standard");

    // Blank -> standard
    assert_eq!(resolve_review_mode(Some("")).unwrap(), "standard");
    assert_eq!(resolve_review_mode(Some("   ")).unwrap(), "standard");

    // standard -> standard (with trimming)
    assert_eq!(resolve_review_mode(Some("standard")).unwrap(), "standard");
    assert_eq!(resolve_review_mode(Some("  standard  ")).unwrap(), "standard");

    // deep -> deep (with trimming)
    assert_eq!(resolve_review_mode(Some("deep")).unwrap(), "deep");
    assert_eq!(resolve_review_mode(Some("  deep  ")).unwrap(), "deep");

    // Unsupported modes -> ErrorData invalid_params
    let err1 = resolve_review_mode(Some("strict")).unwrap_err();
    assert!(err1.message.contains("Invalid `review_mode`: 'strict'"));
    assert!(err1.message.contains("Only 'standard' and 'deep' are supported."));

    let err2 = resolve_review_mode(Some("max")).unwrap_err();
    assert!(err2.message.contains("Invalid `review_mode`: 'max'"));

    let err3 = resolve_review_mode(Some("careful")).unwrap_err();
    assert!(err3.message.contains("Invalid `review_mode`: 'careful'"));
}

#[test]
fn revise_system_prompt_defines_planner_role_and_feedback_handling() {
    use anthro_bridge_mcp_server::mcp::build_revise_system_prompt;

    let prompt = build_revise_system_prompt();
    assert!(prompt.contains("implementation planner, not an implementation agent"));
    assert!(prompt.contains("revise an existing implementation plan based on user feedback"));
    assert!(prompt.contains("Address the requested revisions and feedback directly and thoroughly"));
    assert!(prompt.contains("Preserve valid, unaffected sections of the previous plan"));
    assert!(prompt.contains("Never claim that a change has already been made"));
    assert!(prompt.contains("Never invent or fabricate test results"));
}

#[test]
fn revise_user_prompt_formats_sections_reliably() {
    use anthro_bridge_mcp_server::mcp::{build_revise_user_prompt, ReviseParams};

    let params = ReviseParams {
        task: "Update authentication flow".to_string(),
        current_plan: "1. Add JWT check\n2. Redirect on expiry".to_string(),
        feedback: "Do not use JWT; use session cookies instead".to_string(),
        context: Some("AuthContext.tsx uses React Context".to_string()),
        constraints: Some("Preserve backward compatibility".to_string()),
        plan_context: None,
        frozen_plan: None,
    };

    let prompt = build_revise_user_prompt(&params);
    assert!(prompt.contains("## Task\n\nUpdate authentication flow"));
    assert!(prompt.contains("## Current Implementation Plan\n\n1. Add JWT check\n2. Redirect on expiry"));
    assert!(prompt.contains("## Requested Revisions and Feedback\n\nDo not use JWT; use session cookies instead"));
    assert!(prompt.contains("## Repository context\n\nAuthContext.tsx uses React Context"));
    assert!(prompt.contains("## Constraints\n\nPreserve backward compatibility"));
}
