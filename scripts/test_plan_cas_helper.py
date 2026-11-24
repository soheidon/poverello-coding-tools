"""Focused regression tests for the file-based pov-plan-only helper."""

from __future__ import annotations

import importlib.util
import hashlib
import io
import json
import multiprocessing
import os
import tempfile
import threading
import time
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from types import SimpleNamespace


REPO = Path(__file__).resolve().parents[1]
HELPER_PATH = REPO / "gui/src-tauri/resources/codex-skills/pov-plan-only/scripts/plan_cas_helper.py"
SPEC = importlib.util.spec_from_file_location("plan_cas_helper", HELPER_PATH)
helper = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader
SPEC.loader.exec_module(helper)


def _hold_plan_lock(plan_path: str, appdata: str, ready, release) -> None:
    os.environ["APPDATA"] = appdata
    with helper.plan_mutation_lock(plan_path):
        ready.set()
        release.wait(5)


class PlanCasHelperTests(unittest.TestCase):
    def test_audit_append_preserves_checkpoint_and_following_content(self) -> None:
        source = (
            "# Plan\n\nPlan body.\n\n## 5. Execution History & Audit Trail\n\n"
            "| Timestamp | Phase | Actor | Result | Notes |\n|---|---|---|---|---|\n"
            "| old | plan | codex | done | retained |\n\n"
            "## Poverello Execution Checkpoint\n\n"
            f"{helper.CHECKPOINT_START}\n- Status: IN_PROGRESS\n{helper.CHECKPOINT_END}\n\n"
            "## Appendix\n\nMust survive.\n"
        )
        result = helper.append_audit_row(source, "| new | review | codex | approved | added |")
        self.assertIn("Plan body.", result)
        self.assertIn("| old | plan | codex | done | retained |", result)
        self.assertIn("| new | review | codex | approved | added |", result)
        self.assertIn("- Status: IN_PROGRESS", result)
        self.assertIn("## Appendix\n\nMust survive.", result)

    def test_checkpoint_upsert_appends_checkpoint_after_existing_body(self) -> None:
        source = (
            "# Plan\n\nBody stays.\n\n## Poverello Execution Checkpoint\n\n"
            f"{helper.CHECKPOINT_START}\n- Status: OLD\n{helper.CHECKPOINT_END}\n\n"
            "## Appendix\n\nTail stays.\n"
        )
        result = helper.upsert_checkpoint(source, f"{helper.CHECKPOINT_START}\n- Status: NEW\n{helper.CHECKPOINT_END}")
        self.assertEqual(result.count(helper.CHECKPOINT_START), 2)
        self.assertIn("- Status: NEW", result)
        self.assertIn("- Status: OLD", result)
        self.assertIn("Body stays.", result)
        self.assertIn("## Appendix\n\nTail stays.", result)

    def test_malformed_checkpoint_is_classified_without_truncation(self) -> None:
        malformed = "# Plan\n\n<!-- POVERELLO-CHECKPOINT:START -->\n"
        self.assertEqual(helper.classify_checkpoint(malformed.encode("utf-8"))["checkpoint_state"], "MALFORMED_OR_DUPLICATE")

    def test_audit_row_is_inserted_before_next_section_heading(self) -> None:
        source = (
            "# Plan\n\n## Execution History & Audit Trail\n\n| existing | row |\n\n"
            "## Notes\n\nThis section must stay separate.\n\n"
            "## Poverello Execution Checkpoint\n\n"
            f"{helper.CHECKPOINT_START}\n- Status: ACTIVE\n{helper.CHECKPOINT_END}\n"
        )
        result = helper.append_audit_row(source, "| new | audit | row |")
        self.assertLess(result.index("| new | audit | row |"), result.index("## Notes"))
        self.assertIn("| new | audit | row |\n\n## Notes", result)
        self.assertIn("## Notes\n\nThis section must stay separate.", result)

    def test_candidate_cannot_drop_current_audit_or_checkpoint(self) -> None:
        current = (
            "# Original plan\n\n## 5. Execution History & Audit Trail\n\nold audit\n\n"
            "## Poverello Execution Checkpoint\n\n"
            f"{helper.CHECKPOINT_START}\n- Status: ACTIVE\n{helper.CHECKPOINT_END}\n"
        )
        candidate = "# Revised plan\n\nNew content only.\n"
        merged = helper.preserve_authoritative_history_in_candidate(current, candidate)
        self.assertIn("# Revised plan\n\nNew content only.", merged)
        self.assertIn("old audit", merged)
        self.assertNotIn(helper.CHECKPOINT_START, merged)

    def test_candidate_preserves_empty_and_narrative_audit_sections(self) -> None:
        sections = (
            "## 5. Execution History & Audit Trail\n",
            "## 5. Execution History & Audit Trail\n\n| Timestamp | Phase | Actor | Result | Notes |\n|---|---|---|---|---|\n",
            "## 5. Execution History & Audit Trail\n\nLegacy audit narrative must survive.\n",
            "## 5. Execution History & Audit Trail\n\nLegacy narrative.\n\n| Timestamp | Phase | Actor | Result | Notes |\n|---|---|---|---|---|\n| old | review | codex | done | keep |\n",
        )
        for section in sections:
            with self.subTest(section=section):
                authoritative = "# Current plan\n\nBody.\n\n" + section
                merged = helper.preserve_authoritative_history_in_candidate(authoritative, "# Candidate plan\n\nNew body.\n")
                self.assertIn(section.rstrip(), merged)
                self.assertIn("# Candidate plan\n\nNew body.", merged)

    def test_candidate_preserves_nested_audit_subsections(self) -> None:
        audit = (
            "## 5. Execution History & Audit Trail\n\n"
            "| Timestamp | Phase | Actor | Result | Notes |\n|---|---|---|---|---|\n"
            "| old | review | codex | done | retain |\n\n"
            "### Legacy details\n\nMUST KEEP this nested audit history.\n"
        )
        authoritative = "# Current\n\nBody.\n\n" + audit + "\n## Appendix\n\nTail.\n"
        merged = helper.preserve_authoritative_history_in_candidate(authoritative, "# Candidate\n\nNew body.\n")
        self.assertIn("### Legacy details\n\nMUST KEEP this nested audit history.", merged)
        self.assertIn("| old | review | codex | done | retain |", merged)

        altered = "# Candidate\n\nNew body.\n\n" + audit.replace("MUST KEEP", "ALTERED")
        with self.assertRaises(SystemExit):
            helper.preserve_authoritative_history_in_candidate(authoritative, altered)

    def test_checkpoint_binding_rejects_missing_or_different_target(self) -> None:
        body = "# Plan\n\nContent.\n"
        base_body = body.rstrip("\n")
        revision = helper.canonical_plan_revision(base_body.encode("utf-8"))
        cp = helper.render_checkpoint_block(
            status="IN_PROGRESS", last_completed="start", next_action="draft",
            repo_root=r"C:\\repo", plan_path=".plan/one.md", plan_revision=revision,
            workflow_run_id="run-one", operation_id="op-one", timestamp="2026-11-24T00:00:00Z",
        )
        text = helper.upsert_checkpoint(base_body, cp)
        classified = helper.classify_checkpoint(text.encode("utf-8"))
        self.assertEqual(helper.bind_checkpoint_to_target(classified, r"C:\repo", ".plan/one.md")["checkpoint_state"], "RESUMABLE")

        other_plan = helper.bind_checkpoint_to_target(helper.classify_checkpoint(text.encode("utf-8")), r"C:\repo", ".plan/two.md")
        self.assertEqual(other_plan["checkpoint_state"], "TARGET_MISMATCH")
        self.assertFalse(other_plan["can_resume"])

        missing = text.replace("- Workflow Run ID: run-one\n", "")
        missing_state = helper.bind_checkpoint_to_target(helper.classify_checkpoint(missing.encode("utf-8")), r"C:\repo", ".plan/one.md")
        self.assertEqual(missing_state["checkpoint_state"], "MISSING_BINDING")

    def test_checkpoint_workflow_is_scoped_to_allowlisted_skill(self) -> None:
        body = "# Plan\n\nContent.\n"
        base_body = body.rstrip("\n")
        revision = helper.canonical_plan_revision(base_body.encode("utf-8"))
        for workflow in ("pov-plan-only", "pov-build-review", "pov-full-cycle"):
            with self.subTest(workflow=workflow):
                cp = helper.render_checkpoint_block(
                    workflow=workflow, status="IN_PROGRESS", last_completed="start", next_action="continue",
                    repo_root=r"C:\\repo", plan_path=".plan/one.md", plan_revision=revision,
                    workflow_run_id=f"run-{workflow}", operation_id="op-one", timestamp="2026-11-24T00:00:00Z",
                )
                text = helper.upsert_checkpoint(base_body, cp)
                self.assertEqual(
                    helper.classify_checkpoint(text.encode("utf-8"), workflow)["checkpoint_state"], "RESUMABLE"
                )
                other_workflow = next(item for item in helper.WORKFLOW_NAMES if item != workflow)
                other_state = helper.classify_checkpoint(text.encode("utf-8"), other_workflow)
                self.assertEqual(other_state["checkpoint_state"], "DIFFERENT_WORKFLOW")
                self.assertTrue(other_state["can_start_fresh"])
                self.assertFalse(other_state["can_resume"])

        with self.assertRaises(SystemExit):
            helper.classify_checkpoint(b"# Plan", "pov-unknown")

    def test_new_workflow_checkpoint_starts_at_correct_first_action(self) -> None:
        self.assertEqual(helper.initial_next_action("pov-plan-only"), "draft_plan")
        self.assertEqual(helper.initial_next_action("pov-build-review"), "implement")
        self.assertEqual(helper.initial_next_action("pov-full-cycle"), "draft_plan")
        with self.assertRaises(SystemExit):
            helper.initial_next_action("pov-unknown")

    def test_role_validation_requires_authoritative_codex_assignment(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            old = os.environ.get("CODEX_HOME")
            old_profile = os.environ.get("USERPROFILE")
            old_home = os.environ.get("HOME")
            os.environ["CODEX_HOME"] = tmp
            try:
                os.environ.pop("CODEX_HOME", None)
                os.environ.pop("USERPROFILE", None)
                os.environ.pop("HOME", None)
                with self.assertRaises(SystemExit):
                    helper.validate_codex_roles()
                if old_profile is not None:
                    os.environ["USERPROFILE"] = old_profile
                if old_home is not None:
                    os.environ["HOME"] = old_home
                os.environ["CODEX_HOME"] = tmp
                settings = Path(tmp) / "pov-review-settings.json"
                settings.write_text(json.dumps({"workflow_role_assignments": {"codex": {
                    "planner_id": "codex_session", "reviser_id": "codex_session"
                }}}), encoding="utf-8")
                helper.validate_codex_roles()
                settings.write_text(json.dumps({"role_assignments": {"codex": {
                    "planner_id": "codex_session", "reviser_id": "codex_session"
                }}}), encoding="utf-8")
                with self.assertRaises(SystemExit):
                    helper.validate_codex_roles()
                settings.write_text("{not-json", encoding="utf-8")
                with self.assertRaises(SystemExit):
                    helper.validate_codex_roles()
                settings.write_text(json.dumps({"workflow_role_assignments": {"codex": {
                    "planner_id": "", "reviser_id": "codex_session"
                }}}), encoding="utf-8")
                with self.assertRaises(SystemExit):
                    helper.validate_codex_roles()

                # Match the Rust app: with CODEX_HOME unset, use USERPROFILE/.codex.
                os.environ.pop("CODEX_HOME", None)
                profile_home = Path(tmp) / "profile"
                fallback_home = profile_home / ".codex"
                fallback_home.mkdir(parents=True)
                (fallback_home / "pov-review-settings.json").write_text(json.dumps({
                    "workflow_role_assignments": {"codex": {
                        "planner_id": "codex_session", "reviser_id": "codex"
                    }}
                }), encoding="utf-8")
                prior_profile = os.environ.get("USERPROFILE")
                os.environ["USERPROFILE"] = str(profile_home)
                self.assertEqual(helper.validate_codex_roles(), {
                    "planner_id": "codex_session", "reviser_id": "codex"
                })
                if prior_profile is None:
                    os.environ.pop("USERPROFILE", None)
                else:
                    os.environ["USERPROFILE"] = prior_profile
            finally:
                if old is None:
                    os.environ.pop("CODEX_HOME", None)
                else:
                    os.environ["CODEX_HOME"] = old

    def test_commit_preserves_plan_tail_and_updates_checkpoint_under_lock(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "repo"
            plan_dir = root / ".plan"
            codex_home = Path(tmp) / "codex"
            appdata = Path(tmp) / "appdata"
            plan_dir.mkdir(parents=True)
            codex_home.mkdir()
            appdata.mkdir()
            (root / ".git").mkdir()
            plan_path = plan_dir / "test.md"
            mirror_path = plan_dir / ".active-plan.json"
            registry_path = Path(tmp) / "project-registry.json"
            (codex_home / "pov-review-settings.json").write_text(json.dumps({
                "workflow_role_assignments": {"codex": {
                    "planner_id": "codex_session", "reviser_id": "codex_session"
                }}
            }), encoding="utf-8")
            body = (
                "# Plan\n\nMain content must survive.\n\n## 5. Execution History & Audit Trail\n\n"
                "| Timestamp | Phase | Actor | Result | Notes |\n|---|---|---|---|---|\n"
                "| before | plan | codex | created | retain |\n\n## Appendix\n\nTail must survive.\n"
            )
            revision = helper.canonical_plan_revision(body.rstrip("\n").encode("utf-8"))
            original = helper.upsert_checkpoint(body, (
                f"{helper.CHECKPOINT_START}\n- Workflow: pov-plan-only\n- Status: IN_PROGRESS\n"
                f"- Repository: `{root}`\n- Active Plan: `.plan/test.md`\n- Workflow Run ID: run-test\n"
                f"- Plan Revision: sha256:{revision}\n{helper.CHECKPOINT_END}"
            ))
            plan_path.write_text(original, encoding="utf-8", newline="\n")
            mirror_path.write_text(json.dumps({"surfaces": {"codex": {"active_plan_path": ".plan/test.md"}}}), encoding="utf-8")
            registry_path.write_text(json.dumps({"projects": [{
                "project_id": "project-test",
                "repo_root": str(root),
                "plan_paths": {"codex": ".plan/test.md"},
            }]}), encoding="utf-8")
            old_env = {key: os.environ.get(key) for key in ("APPDATA", "CODEX_HOME", "POVERELLO_REGISTRY_PATH")}
            os.environ["APPDATA"] = str(appdata)
            os.environ["CODEX_HOME"] = str(codex_home)
            os.environ["POVERELLO_REGISTRY_PATH"] = str(registry_path)
            try:
                expected = hashlib.sha256(original.encode("utf-8")).hexdigest()
                # A revision-valid checkpoint copied to another repository/plan must be rejected
                # by the real resolver and by both read-only verification and commit.
                for old_value, new_value, expected_state in (
                    (f"- Repository: `{root}`", f"- Repository: `{Path(tmp) / 'other-repo'}`", "TARGET_MISMATCH"),
                    ("- Active Plan: `.plan/test.md`", "- Active Plan: `.plan/other.md`", "TARGET_MISMATCH"),
                    (f"- Repository: `{root}`", "- Repository: ` `", "MISSING_BINDING"),
                    ("- Active Plan: `.plan/test.md`", "- Active Plan: ` `", "MISSING_BINDING"),
                ):
                    tampered = original.replace(old_value, new_value)
                    self.assertNotEqual(tampered, original)
                    plan_path.write_text(tampered, encoding="utf-8", newline="\n")
                    resolved_tampered = helper.resolve_plan(str(root), "codex", None)
                    self.assertEqual(resolved_tampered["checkpoint_state"], expected_state)
                    verify_args = SimpleNamespace(repo=str(root), surface="codex", plan=None)
                    before_failed_commit = plan_path.read_bytes()
                    with redirect_stderr(io.StringIO()), redirect_stdout(io.StringIO()), self.assertRaises(SystemExit):
                        helper.cmd_verify(verify_args)
                    blocked_commit = SimpleNamespace(
                        repo=str(root), surface="codex", plan=None,
                        expected_sha256=hashlib.sha256(tampered.encode("utf-8")).hexdigest(), body_file=None,
                        phase="review", actor="codex", result="approved", notes="must reject target mismatch",
                        status="COMPLETED", last_completed="review", next_action="stop",
                        workflow_run_id="run-test", operation_id="op-test",
                    )
                    with redirect_stderr(io.StringIO()), redirect_stdout(io.StringIO()), self.assertRaises(SystemExit):
                        helper.cmd_commit(blocked_commit)
                    self.assertEqual(plan_path.read_bytes(), before_failed_commit)
                    plan_path.write_text(original, encoding="utf-8", newline="\n")

                mismatch_args = SimpleNamespace(
                    repo=str(root), surface="codex", plan=None,
                    expected_sha256=expected, body_file=None,
                    phase="review", actor="codex", result="approved",
                    notes="must reject wrong run", status="COMPLETED",
                    last_completed="review", next_action="stop",
                    workflow_run_id="different-run", operation_id="op-test",
                )
                with redirect_stdout(io.StringIO()), self.assertRaises(SystemExit):
                    helper.cmd_commit(mismatch_args)
                self.assertEqual(plan_path.read_text(encoding="utf-8"), original)

                args = SimpleNamespace(
                    repo=str(root), surface="codex", plan=None,
                    expected_sha256=expected, body_file=None,
                    phase="review", actor="codex", result="approved",
                    notes="focused regression", status="COMPLETED",
                    last_completed="review", next_action="stop",
                    workflow_run_id="run-test", operation_id="op-test",
                )
                with redirect_stdout(io.StringIO()):
                    helper.cmd_commit(args)
                committed = plan_path.read_text(encoding="utf-8")
                self.assertIn("Main content must survive.", committed)
                self.assertIn("| before | plan | codex | created | retain |", committed)
                self.assertIn("| review | codex | approved | focused regression |", committed)
                self.assertIn("## Appendix\n\nTail must survive.", committed)
                self.assertEqual(committed.count(helper.CHECKPOINT_START), 1)
                resolved = helper.resolve_plan(str(root), "codex", None)
                self.assertEqual(resolved["checkpoint"].get("Plan Revision"), f"sha256:{resolved['canonical_plan_revision']}")

                # Legacy/different-workflow identity is historical evidence only: archive it,
                # record its missing/mismatched binding, then create a fresh target-bound run.
                legacy = committed.replace("- Workflow: pov-plan-only", "- Workflow: pov-build-review")
                legacy = legacy.replace(f"- Repository: `{root}`", f"- Repository: `{Path(tmp) / 'legacy-repo'}`")
                legacy = legacy.replace("- Active Plan: `.plan/test.md`\n", "")
                legacy = legacy.replace("- Workflow Run ID: run-test\n", "")
                legacy = legacy.replace(f"- Plan Revision: sha256:{resolved['canonical_plan_revision']}\n", "")
                plan_path.write_text(legacy, encoding="utf-8", newline="\n")
                legacy_args = SimpleNamespace(
                    repo=str(root), surface="codex", plan=None,
                    expected_sha256=hashlib.sha256(legacy.encode("utf-8")).hexdigest(),
                    archive_prior=True, workflow_run_id="fresh-run", operation_id="fresh-op",
                    actor="codex", notes="archive legacy binding",
                )
                with redirect_stdout(io.StringIO()) as start_out:
                    helper.cmd_start(legacy_args)
                started = json.loads(start_out.getvalue())
                self.assertTrue(started["started"])
                self.assertEqual(started["workflow_run_id"], "fresh-run")
                restarted = helper.resolve_plan(str(root), "codex", None)
                self.assertEqual(restarted["checkpoint_state"], "RESUMABLE")
                self.assertEqual(restarted["checkpoint"].get("Repository"), f"`{root}`")
                self.assertEqual(restarted["checkpoint"].get("Active Plan"), "`.plan/test.md`")
                final_text = plan_path.read_text(encoding="utf-8")
                self.assertIn("prior_binding=untrusted(missing=Active Plan,Workflow Run ID;mismatched=Repository)", final_text)
                self.assertIn("### Archived Prior Checkpoint", final_text)
            finally:
                for key, value in old_env.items():
                    if value is None:
                        os.environ.pop(key, None)
                    else:
                        os.environ[key] = value

    @unittest.skipUnless(os.name == "nt", "Windows lock interoperability is platform-specific")
    def test_shared_plan_lock_serializes_processes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            appdata = os.path.join(tmp, "appdata")
            os.makedirs(appdata)
            plan_path = os.path.join(tmp, "plan.md")
            Path(plan_path).write_text("# Plan\n", encoding="utf-8")
            ctx = multiprocessing.get_context("spawn")
            ready, release = ctx.Event(), ctx.Event()
            child = ctx.Process(target=_hold_plan_lock, args=(plan_path, appdata, ready, release))
            child.start()
            try:
                self.assertTrue(ready.wait(5), "child failed to acquire the plan lock")
                acquired = threading.Event()

                def _acquire_in_parent() -> None:
                    os.environ["APPDATA"] = appdata
                    with helper.plan_mutation_lock(plan_path):
                        acquired.set()

                thread = threading.Thread(target=_acquire_in_parent, daemon=True)
                thread.start()
                time.sleep(0.15)
                self.assertFalse(acquired.is_set(), "second process bypassed the shared plan lock")
                release.set()
                thread.join(5)
                self.assertTrue(acquired.is_set(), "second process did not acquire lock after release")
            finally:
                release.set()
                child.join(5)
                if child.is_alive():
                    child.terminate()
                    child.join(2)


if __name__ == "__main__":
    unittest.main(verbosity=2)
