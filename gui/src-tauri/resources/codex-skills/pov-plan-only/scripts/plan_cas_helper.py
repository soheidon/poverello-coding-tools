#!/usr/bin/env python3
"""Deterministic file-based plan resolver, workflow starter, CAS commit, and resumption verifier for pov-plan-only."""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import re
import stat
import sys
import uuid

CHECKPOINT_HEADER = "## Poverello Execution Checkpoint"
CHECKPOINT_START = "<!-- POVERELLO-CHECKPOINT:START -->"
CHECKPOINT_END = "<!-- POVERELLO-CHECKPOINT:END -->"


def fail(code: str, message: str) -> None:
    sys.stderr.write(f"BLOCKED — {code}: {message}\n")
    sys.exit(2)


def check_no_reparse(path: str) -> None:
    if not os.path.lexists(path):
        return
    if os.path.islink(path):
        fail("REPARSE_POINT_REJECTED", f"Symlink rejected: {path}")
    st = os.lstat(path)
    attrs = getattr(st, "st_file_attributes", 0)
    if attrs & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0):
        fail("REPARSE_POINT_REJECTED", f"Reparse point rejected: {path}")


def check_components_inside_repo(repo_root: str, target_abs: str) -> None:
    real_repo = os.path.realpath(repo_root)
    real_target = os.path.realpath(target_abs)
    try:
        common = os.path.commonpath([real_repo, real_target])
    except ValueError:
        fail("PATH_OUTSIDE_REPO", f"Path escapes repository: {target_abs}")
    if os.path.normcase(common) != os.path.normcase(real_repo):
        fail("PATH_OUTSIDE_REPO", f"Path escapes repository: {target_abs}")
    rel = os.path.relpath(target_abs, repo_root)
    if rel.startswith("..") or os.path.isabs(rel):
        fail("PATH_OUTSIDE_REPO", f"Relative path escapes repository: {rel}")
    cur = repo_root
    check_no_reparse(cur)
    for part in rel.split(os.sep):
        if not part or part == ".":
            continue
        cur = os.path.join(cur, part)
        check_no_reparse(cur)


def find_repo_root(start_dir: str) -> str:
    cur = os.path.abspath(start_dir)
    while True:
        check_no_reparse(cur)
        git_entry = os.path.join(cur, ".git")
        if os.path.lexists(git_entry):
            return cur
        parent = os.path.dirname(cur)
        if parent == cur:
            fail("NO_GIT_REPO", f"No enclosing Git repository found from {start_dir}")
        cur = parent


def registry_path() -> str:
    override = os.environ.get("POVERELLO_REGISTRY_PATH")
    if override:
        return override
    appdata = os.environ.get("APPDATA", "")
    if not appdata:
        fail("REGISTRY_UNAVAILABLE", "APPDATA is not set; cannot locate Stable project-registry.json")
    return os.path.join(appdata, "Poverello", "project-registry.json")


def load_registry() -> dict:
    rpath = registry_path()
    check_no_reparse(rpath)
    if not os.path.isfile(rpath):
        fail("REGISTRY_UNAVAILABLE", f"Project registry not found: {rpath}")
    try:
        with open(rpath, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception as exc:
        fail("REGISTRY_UNAVAILABLE", f"Cannot read project registry {rpath}: {exc}")


def validate_codex_roles() -> dict:
    """Read the authoritative Codex planner/reviser assignment; never fall back to the repo mirror."""
    codex_home = os.environ.get("CODEX_HOME")
    if not codex_home:
        profile = os.environ.get("USERPROFILE") if os.name == "nt" else os.environ.get("HOME")
        if not profile:
            fail("ROLE_SETTINGS_UNAVAILABLE", "Cannot determine Codex home because CODEX_HOME and the user home are unset")
        codex_home = os.path.join(profile, ".codex")
    settings_path = os.path.join(codex_home, "pov-review-settings.json")
    check_no_reparse(settings_path)
    try:
        with open(settings_path, "r", encoding="utf-8") as f:
            settings = json.load(f)
    except FileNotFoundError:
        fail("ROLE_SETTINGS_UNAVAILABLE", "Codex pov-review-settings.json was not found")
    except Exception:
        fail("ROLE_SETTINGS_INVALID", "Codex pov-review-settings.json is unreadable or invalid JSON")
    assignments = settings.get("workflow_role_assignments") if isinstance(settings, dict) else None
    codex = assignments.get("codex") if isinstance(assignments, dict) else None
    if not isinstance(codex, dict):
        fail("ROLE_ASSIGNMENT_MISSING", "Codex planner/reviser assignment is missing")
    planner = codex.get("planner_id")
    reviser = codex.get("reviser_id")
    allowed = {"codex_session", "codex"}
    if planner not in allowed or reviser not in allowed:
        fail("ROLE_MISMATCH", "Codex planner and reviser must both be assigned to codex_session or codex")
    return {"planner_id": planner, "reviser_id": reviser}


def normalize_rel(p: str) -> str:
    s = p.strip().replace("\\", "/")
    while s.startswith("./"):
        s = s[2:]
    return s.lstrip("/")


def strip_verbatim(p: str) -> str:
    if p.startswith("\\\\?\\UNC\\"):
        return "\\\\" + p[8:]
    if p.startswith("\\\\?\\"):
        return p[4:]
    return p


def scan_live_checkpoint_markers(norm_text: str) -> tuple[str, tuple[int, int, int] | None]:
    """
    Scans normalized LF text for standalone checkpoint markers outside fenced code blocks.
    Returns (status, (section_start, start_marker_idx, end_marker_idx)) where status is:
      - "NONE": no live markers found
      - "VALID": exactly one complete START..END pair found
      - "MALFORMED": duplicate, unbalanced, or inverted markers found
    """
    fence: tuple[str, int] | None = None
    start_idx: int | None = None
    end_idx: int | None = None
    offset = 0

    for line in norm_text.splitlines(keepends=True):
        trimmed_left = line.lstrip(" \t")
        if trimmed_left and trimmed_left[0] in ("`", "~"):
            ch = trimmed_left[0]
            run_len = len(trimmed_left) - len(trimmed_left.lstrip(ch))
            if run_len >= 3:
                if fence is None:
                    fence = (ch, run_len)
                elif fence[0] == ch and run_len >= fence[1]:
                    fence = None
        elif fence is None:
            stripped = line.strip()
            if stripped == CHECKPOINT_START:
                if start_idx is not None:
                    return ("MALFORMED", None)
                start_idx = offset + line.find(CHECKPOINT_START)
            elif stripped == CHECKPOINT_END:
                if end_idx is not None:
                    return ("MALFORMED", None)
                end_idx = offset + line.find(CHECKPOINT_END)
        offset += len(line)

    if start_idx is None and end_idx is None:
        return ("NONE", None)
    if start_idx is not None and end_idx is not None and start_idx < end_idx:
        hdr_idx = norm_text.rfind(CHECKPOINT_HEADER, 0, start_idx)
        if hdr_idx != -1 and norm_text[hdr_idx + len(CHECKPOINT_HEADER) : start_idx].strip() == "":
            sec_start = hdr_idx
        else:
            sec_start = start_idx
        return ("VALID", (sec_start, start_idx, end_idx))
    return ("MALFORMED", None)


def canonical_plan_revision(raw_bytes: bytes) -> str:
    try:
        text = raw_bytes.decode("utf-8")
    except UnicodeDecodeError as exc:
        fail("INVALID_UTF8", f"Plan file is not valid UTF-8: {exc}")
    if text.startswith("\ufeff"):
        text = text[1:]
    norm = text.replace("\r\n", "\n").replace("\r", "\n")
    marker_status, span = scan_live_checkpoint_markers(norm)
    if marker_status == "MALFORMED":
        fail("MALFORMED_MARKERS", "Plan contains duplicate or unbalanced checkpoint markers")
    if marker_status == "VALID" and span is not None:
        sec_start, _, end_idx = span
        after_end = end_idx + len(CHECKPOINT_END)
        prefix = norm[:sec_start].rstrip("\n")
        suffix = norm[after_end:].lstrip("\n")
        if prefix and suffix:
            rev_body = f"{prefix}\n\n{suffix}"
        else:
            rev_body = prefix or suffix
    else:
        rev_body = norm
    return hashlib.sha256(rev_body.encode("utf-8")).hexdigest()


def extract_checkpoint_fields(norm_text: str) -> tuple[str, dict[str, str], str | None]:
    marker_status, span = scan_live_checkpoint_markers(norm_text)
    if marker_status == "NONE":
        return ("NONE", {}, None)
    if marker_status == "MALFORMED" or span is None:
        return ("MALFORMED", {}, None)
    sec_start, start_idx, end_idx = span
    block = norm_text[start_idx + len(CHECKPOINT_START) : end_idx]
    out: dict[str, str] = {}
    for line in block.splitlines():
        m = re.match(r"^\s*-\s*([^:]+):\s*(.*)$", line)
        if m:
            out[m.group(1).strip()] = m.group(2).strip()
    raw_section = norm_text[sec_start : end_idx + len(CHECKPOINT_END)]
    return ("VALID", out, raw_section)


def classify_checkpoint(raw_bytes: bytes) -> dict:
    try:
        text = raw_bytes.decode("utf-8")
    except UnicodeDecodeError as exc:
        fail("INVALID_UTF8", f"Plan file is not valid UTF-8: {exc}")
    if text.startswith("\ufeff"):
        text = text[1:]
    norm = text.replace("\r\n", "\n").replace("\r", "\n")
    marker_status, fields, raw_section = extract_checkpoint_fields(norm)

    if marker_status == "MALFORMED":
        return {
            "checkpoint_state": "MALFORMED_OR_DUPLICATE",
            "can_start_fresh": False,
            "can_resume": False,
            "prior_workflow": None,
            "has_plan_revision": False,
            "checkpoint": {},
            "canonical_plan_revision": None,
            "raw_section": None,
        }

    rev = canonical_plan_revision(raw_bytes)
    if marker_status == "NONE":
        return {
            "checkpoint_state": "NONE",
            "can_start_fresh": True,
            "can_resume": False,
            "prior_workflow": None,
            "has_plan_revision": False,
            "checkpoint": {},
            "canonical_plan_revision": rev,
            "raw_section": None,
        }

    prior_wf = fields.get("Workflow", "").strip()
    status_val = fields.get("Status", "").strip().upper()
    plan_rev_raw = fields.get("Plan Revision", "").strip()
    has_rev = bool(plan_rev_raw)
    cp_rev = plan_rev_raw.removeprefix("sha256:").strip()

    if status_val == "HALTED":
        state = "HALTED"
        can_start = False
        can_resume = False
    elif prior_wf != "pov-plan-only":
        state = "DIFFERENT_WORKFLOW"
        can_start = True
        can_resume = False
    elif not has_rev:
        state = "MISSING_PLAN_REVISION"
        can_start = True
        can_resume = False
    elif cp_rev.lower() != rev.lower():
        state = "REVISION_MISMATCH"
        can_start = False
        can_resume = False
    else:
        state = "RESUMABLE"
        can_start = False
        can_resume = True

    return {
        "checkpoint_state": state,
        "can_start_fresh": can_start,
        "can_resume": can_resume,
        "prior_workflow": prior_wf or None,
        "has_plan_revision": has_rev,
        "checkpoint": fields,
        "canonical_plan_revision": rev,
        "raw_section": raw_section,
    }


def _unquote_checkpoint_value(value: str) -> str:
    value = value.strip()
    if len(value) >= 2 and value[0] == "`" and value[-1] == "`":
        return value[1:-1].strip()
    return value


def bind_checkpoint_to_target(cp_info: dict, repo_root: str, plan_path: str) -> dict:
    """Fail closed when a resumable pov-plan-only checkpoint lacks or mismatches its target identity."""
    if cp_info.get("checkpoint_state") not in ("RESUMABLE", "REVISION_MISMATCH", "HALTED"):
        return cp_info
    if cp_info.get("prior_workflow") != "pov-plan-only":
        return cp_info
    fields = cp_info.get("checkpoint", {})
    stored_repo = _unquote_checkpoint_value(fields.get("Repository", ""))
    stored_plan = _unquote_checkpoint_value(fields.get("Active Plan", ""))
    stored_run = fields.get("Workflow Run ID", "").strip()
    if not stored_repo or not stored_plan or not stored_run:
        cp_info["checkpoint_state"] = "MISSING_BINDING"
        cp_info["can_start_fresh"] = False
        cp_info["can_resume"] = False
        cp_info["binding_error"] = "Checkpoint lacks Repository, Active Plan, or Workflow Run ID"
        return cp_info
    try:
        repo_matches = os.path.normcase(os.path.realpath(strip_verbatim(stored_repo))) == os.path.normcase(
            os.path.realpath(strip_verbatim(repo_root))
        )
    except (OSError, ValueError):
        repo_matches = False
    try:
        recorded_rel = normalize_rel(stored_plan)
        target_rel = normalize_rel(plan_path)
        plan_matches = os.path.normcase(recorded_rel) == os.path.normcase(target_rel)
    except (OSError, ValueError):
        plan_matches = False
    if not repo_matches or not plan_matches:
        cp_info["checkpoint_state"] = "TARGET_MISMATCH"
        cp_info["can_start_fresh"] = False
        cp_info["can_resume"] = False
        cp_info["binding_error"] = "Checkpoint Repository or Active Plan does not match the resolved target"
    return cp_info


def describe_prior_checkpoint_binding(cp_info: dict, repo_root: str, plan_path: str) -> str:
    """Describe legacy identity evidence for audit; never use it to authorize a fresh archive-start."""
    fields = cp_info.get("checkpoint", {})
    missing = [key for key in ("Repository", "Active Plan", "Workflow Run ID") if not _unquote_checkpoint_value(fields.get(key, ""))]
    mismatched: list[str] = []
    stored_repo = _unquote_checkpoint_value(fields.get("Repository", ""))
    stored_plan = _unquote_checkpoint_value(fields.get("Active Plan", ""))
    if stored_repo:
        try:
            if os.path.normcase(os.path.realpath(strip_verbatim(stored_repo))) != os.path.normcase(os.path.realpath(strip_verbatim(repo_root))):
                mismatched.append("Repository")
        except (OSError, ValueError):
            mismatched.append("Repository")
    if stored_plan:
        try:
            if os.path.normcase(normalize_rel(stored_plan)) != os.path.normcase(normalize_rel(plan_path)):
                mismatched.append("Active Plan")
        except (OSError, ValueError):
            mismatched.append("Active Plan")
    if missing or mismatched:
        parts = []
        if missing:
            parts.append("missing=" + ",".join(missing))
        if mismatched:
            parts.append("mismatched=" + ",".join(mismatched))
        return "prior_binding=untrusted(" + ";".join(parts) + ")"
    return "prior_binding=observed_match_not_used_as_authority"


def resolve_plan(repo_dir: str, surface: str, explicit_plan: str | None) -> dict:
    codex_roles = validate_codex_roles() if surface == "codex" else None
    repo_root = find_repo_root(repo_dir)
    real_repo = os.path.normcase(os.path.realpath(strip_verbatim(repo_root)))

    plan_dir = os.path.join(repo_root, ".plan")
    check_no_reparse(plan_dir)
    mirror_path = os.path.join(plan_dir, ".active-plan.json")
    check_no_reparse(mirror_path)

    reg = load_registry()
    matches = []
    for proj in reg.get("projects", []):
        pr = proj.get("repo_root", "")
        if not pr:
            continue
        real_pr = os.path.normcase(os.path.realpath(strip_verbatim(pr)))
        if real_pr == real_repo:
            matches.append(proj)
    if len(matches) == 0:
        fail("UNREGISTERED_REPO", f"Repository is not registered in Stable project registry: {repo_root}")
    if len(matches) > 1:
        fail("AMBIGUOUS_REGISTRY", f"Multiple project entries match repository: {repo_root}")

    proj = matches[0]
    project_id = proj.get("project_id", "")
    plan_paths = proj.get("plan_paths") or {}
    reg_rel = plan_paths.get(surface) if isinstance(plan_paths, dict) else None
    if not reg_rel and surface == "codex":
        reg_rel = proj.get("plan_path")
    if not reg_rel:
        fail("NO_REGISTERED_PLAN", f"No plan registered for surface '{surface}' in project {project_id}")
    reg_rel_norm = normalize_rel(reg_rel)

    mirror_data = None
    mirror_rel_norm = None
    if os.path.isfile(mirror_path):
        try:
            with open(mirror_path, "r", encoding="utf-8") as f:
                mirror_data = json.load(f)
        except Exception as exc:
            fail("INVALID_MIRROR", f"Cannot parse {mirror_path}: {exc}")
        surf_obj = (mirror_data.get("surfaces") or {}).get(surface)
        if isinstance(surf_obj, dict):
            if "active_plan_path" in surf_obj and surf_obj["active_plan_path"] is None:
                fail("SURFACE_CLEARED", f"Surface '{surface}' was explicitly cleared in .active-plan.json")
            if surf_obj.get("active_plan_path"):
                mirror_rel_norm = normalize_rel(surf_obj["active_plan_path"])
        elif mirror_data.get("active_plan_path"):
            mirror_rel_norm = normalize_rel(mirror_data["active_plan_path"])

    if explicit_plan:
        abs_candidate = explicit_plan if os.path.isabs(explicit_plan) else os.path.join(repo_root, explicit_plan)
        check_components_inside_repo(repo_root, abs_candidate)
        cand_rel_norm = normalize_rel(os.path.relpath(os.path.realpath(abs_candidate), os.path.realpath(repo_root)))
        if cand_rel_norm != reg_rel_norm:
            fail(
                "EXPLICIT_PLAN_MISMATCH",
                f"Explicit plan '{cand_rel_norm}' does not match Stable registry '{reg_rel_norm}'",
            )
        if mirror_rel_norm is not None and mirror_rel_norm != reg_rel_norm:
            fail(
                "MIRROR_REGISTRY_MISMATCH",
                f"Repo mirror '{mirror_rel_norm}' disagrees with Stable registry '{reg_rel_norm}'; resync in Poverello",
            )
        chosen_rel = cand_rel_norm
    else:
        if mirror_rel_norm is None:
            fail("NO_MIRROR_SELECTION", f"No active plan mirror for surface '{surface}' in {mirror_path}")
        if mirror_rel_norm != reg_rel_norm:
            fail(
                "MIRROR_REGISTRY_MISMATCH",
                f"Repo mirror '{mirror_rel_norm}' disagrees with Stable registry '{reg_rel_norm}'; resync in Poverello",
            )
        chosen_rel = mirror_rel_norm

    if not chosen_rel.lower().endswith(".md"):
        fail("INVALID_PLAN_EXTENSION", f"Plan file must be Markdown (.md): {chosen_rel}")

    abs_plan = os.path.join(repo_root, *chosen_rel.split("/"))
    check_components_inside_repo(repo_root, abs_plan)
    if not os.path.isfile(abs_plan):
        fail("PLAN_MISSING", f"Plan file does not exist: {abs_plan}")

    if surface != "codex" and isinstance(mirror_data, dict):
        roles = (
            (mirror_data.get("project_roles") or {}).get(project_id, {}).get(surface)
            or (mirror_data.get("role_assignments") or {}).get(surface)
        )
        if isinstance(roles, dict):
            planner = roles.get("planner_id", "")
            reviser = roles.get("reviser_id", "")
            allowed = {"codex_session", "codex", ""}
            if planner not in allowed or reviser not in allowed:
                fail("ROLE_MISMATCH", f"Surface '{surface}' role is not assigned to codex_session (planner={planner}, reviser={reviser})")

    with open(abs_plan, "rb") as f:
        raw_bytes = f.read()
    sha256_hash = hashlib.sha256(raw_bytes).hexdigest()
    cp_info = bind_checkpoint_to_target(classify_checkpoint(raw_bytes), os.path.realpath(repo_root), chosen_rel)

    return {
        "project_id": project_id,
        "surface": surface,
        "repo_root": os.path.realpath(repo_root),
        "plan_path": chosen_rel,
        "abs_plan_path": abs_plan,
        "sha256_hash": sha256_hash,
        "canonical_plan_revision": cp_info["canonical_plan_revision"],
        "checkpoint_state": cp_info["checkpoint_state"],
        "can_start_fresh": cp_info["can_start_fresh"],
        "can_resume": cp_info["can_resume"],
        "prior_workflow": cp_info["prior_workflow"],
        "has_plan_revision": cp_info["has_plan_revision"],
        "checkpoint": cp_info["checkpoint"],
        "role_assignments": codex_roles,
        "_raw_section": cp_info["raw_section"],
    }


def strip_live_checkpoint(norm_text: str) -> str:
    marker_status, span = scan_live_checkpoint_markers(norm_text)
    if marker_status == "MALFORMED":
        fail("MALFORMED_MARKERS", "Plan contains duplicate or unbalanced checkpoint markers")
    if marker_status == "VALID" and span is not None:
        sec_start, _, end_idx = span
        after_end = end_idx + len(CHECKPOINT_END)
        prefix = norm_text[:sec_start].rstrip("\n")
        suffix = norm_text[after_end:].lstrip("\n")
        if prefix and suffix:
            return f"{prefix}\n\n{suffix}"
        return prefix or suffix
    return norm_text.rstrip("\n")


def safe_fence_for(text: str) -> str:
    max_ticks = 3
    for m in re.finditer(r"`{3,}", text):
        max_ticks = max(max_ticks, len(m.group(0)))
    return "`" * (max_ticks + 1)


def extract_archived_prior_checkpoints(norm_text: str) -> list[str]:
    lines = norm_text.splitlines(keepends=True)
    blocks: list[str] = []
    in_fence = False
    fence_char = ""
    fence_len = 0
    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.lstrip()
        if not in_fence:
            if re.match(r"^### Archived Prior Checkpoint \(.+\)\s*$", line.rstrip("\n")):
                block_lines = [line]
                j = i + 1
                while j < len(lines) and lines[j].strip() == "":
                    block_lines.append(lines[j])
                    j += 1
                if j < len(lines):
                    fm = re.match(r"^(`{3,}|~{3,})", lines[j].lstrip())
                    if fm:
                        inner_char = fm.group(1)[0]
                        inner_len = len(fm.group(1))
                        block_lines.append(lines[j])
                        j += 1
                        while j < len(lines):
                            block_lines.append(lines[j])
                            cm = re.match(rf"^{re.escape(inner_char)}{{{inner_len},}}\s*$", lines[j].lstrip())
                            if cm:
                                blocks.append("".join(block_lines).rstrip("\n"))
                                i = j + 1
                                break
                            j += 1
                        else:
                            fail("MALFORMED_ARCHIVE", "Unclosed fenced block in Archived Prior Checkpoint")
                        continue
                fail("MALFORMED_ARCHIVE", "Archived Prior Checkpoint heading is not followed by a fenced block")
            fm = re.match(r"^(`{3,}|~{3,})", stripped)
            if fm:
                in_fence = True
                fence_char = fm.group(1)[0]
                fence_len = len(fm.group(1))
        else:
            if re.match(rf"^{re.escape(fence_char)}{{{fence_len},}}\s*$", stripped):
                in_fence = False
                fence_char = ""
                fence_len = 0
        i += 1
    return blocks


def extract_audit_trail_info(norm_text: str) -> dict:
    audit_hdr = re.search(r"^##\s+(?:5\.\s+)?Execution History & Audit Trail\s*$", norm_text, re.MULTILINE)
    if not audit_hdr:
        return {"found": False, "section": None, "data_rows": [], "non_table_body": "", "start": None, "end": None}
    sec_start = audit_hdr.start()
    # A `###` heading is nested content of the audit section and must remain protected.
    next_hdr = re.search(r"^##\s+", norm_text[audit_hdr.end() :], re.MULTILINE)
    sec_end = audit_hdr.end() + next_hdr.start() if next_hdr else len(norm_text)
    section = norm_text[sec_start:sec_end].rstrip("\n")
    data_rows: list[str] = []
    non_table_lines: list[str] = []
    seen_header = False
    for ln in norm_text[audit_hdr.end() : sec_end].splitlines():
        s = ln.strip()
        if not s:
            continue
        if not s.startswith("|"):
            non_table_lines.append(s)
            continue
        if re.match(r"^\|\s*[-:]+(?:\s*\|\s*[-:]+)+\s*\|?\s*$", s):
            seen_header = True
            continue
        if not seen_header and re.match(r"^\|\s*Timestamp\s*\|", s, re.IGNORECASE):
            continue
        data_rows.append(s)
    return {
        "found": True,
        "section": section,
        "data_rows": data_rows,
        "non_table_body": "\n".join(non_table_lines),
        "start": sec_start,
        "end": sec_end,
    }


def preserve_authoritative_history_in_candidate(auth_text: str, cand_text: str) -> str:
    auth_body = strip_live_checkpoint(auth_text)
    cand_body = strip_live_checkpoint(cand_text)

    auth_archives = extract_archived_prior_checkpoints(auth_body)
    cand_archives = extract_archived_prior_checkpoints(cand_body)
    missing_archives: list[str] = []
    if auth_archives:
        if cand_archives:
            if cand_archives != auth_archives:
                fail(
                    "ARCHIVE_MISMATCH",
                    "Candidate body-file altered or partially omitted archived prior checkpoint blocks",
                )
        else:
            missing_archives = auth_archives
    elif cand_archives:
        fail(
            "ARCHIVE_MISMATCH",
            "Candidate body-file contains unexpected archived prior checkpoint blocks",
        )

    auth_audit = extract_audit_trail_info(auth_body)
    cand_audit = extract_audit_trail_info(cand_body)
    has_auth_section = bool(auth_audit["found"])
    archives_blob = "\n\n".join(missing_archives)

    if has_auth_section:
        if not cand_audit["found"]:
            parts = [cand_body.rstrip("\n")]
            if archives_blob:
                parts.append(archives_blob)
            parts.append(auth_audit["section"])
            return "\n\n".join(p for p in parts if p)
        if not cand_audit["data_rows"] and not cand_audit["non_table_body"]:
            before = cand_body[: cand_audit["start"]].rstrip("\n")
            after = cand_body[cand_audit["end"] :].lstrip("\n")
            mid_parts = [p for p in (archives_blob, auth_audit["section"]) if p]
            mid = "\n\n".join(mid_parts)
            res = f"{before}\n\n{mid}" if before else mid
            if after:
                res = f"{res}\n\n{after}"
            return res
        if cand_audit["section"] != auth_audit["section"]:
            fail(
                "AUDIT_HISTORY_MISMATCH",
                "Candidate body-file altered or omitted existing audit trail content",
            )
        if archives_blob:
            before = cand_body[: cand_audit["start"]].rstrip("\n")
            from_audit = cand_body[cand_audit["start"] :].lstrip("\n")
            return f"{before}\n\n{archives_blob}\n\n{from_audit}" if before else f"{archives_blob}\n\n{from_audit}"
        return cand_body

    if archives_blob:
        if cand_audit["found"]:
            before = cand_body[: cand_audit["start"]].rstrip("\n")
            from_audit = cand_body[cand_audit["start"] :].lstrip("\n")
            return f"{before}\n\n{archives_blob}\n\n{from_audit}" if before else f"{archives_blob}\n\n{from_audit}"
        return f"{cand_body.rstrip(chr(10))}\n\n{archives_blob}" if cand_body.strip() else archives_blob
    return cand_body


def append_audit_row(body: str, row: str) -> str:
    audit_hdr = re.search(r"^##\s+(?:5\.\s+)?Execution History & Audit Trail\s*$", body, re.MULTILINE)
    if not audit_hdr:
        return (
            body.rstrip("\n")
            + "\n\n## 5. Execution History & Audit Trail\n\n| Timestamp | Phase | Actor | Result | Notes |\n|---|---|---|---|---|\n"
            + row
        )
    sec_start = audit_hdr.end()
    next_hdr = re.search(r"^(?:##|###)\s+", body[sec_start:], re.MULTILINE)
    sec_end = sec_start + next_hdr.start() if next_hdr else len(body)
    section = body[sec_start:sec_end]
    lines = section.splitlines(keepends=True)
    last_table_idx = None
    for idx, ln in enumerate(lines):
        if ln.lstrip().startswith("|"):
            last_table_idx = idx
    if last_table_idx is not None:
        insert_pos = sec_start + sum(len(l) for l in lines[: last_table_idx + 1])
        prefix = body[:insert_pos].rstrip("\n")
        suffix = body[insert_pos:].lstrip("\n")
        if suffix:
            return f"{prefix}\n{row}\n\n{suffix}"
        return f"{prefix}\n{row}"
    prefix = body[:sec_end].rstrip("\n")
    suffix = body[sec_end:].lstrip("\n")
    table_hdr = "| Timestamp | Phase | Actor | Result | Notes |\n|---|---|---|---|---|"
    if suffix:
        return f"{prefix}\n\n{table_hdr}\n{row}\n\n{suffix}"
    return f"{prefix}\n\n{table_hdr}\n{row}"


def upsert_checkpoint(body: str, cp_md: str) -> str:
    return f"{body.rstrip()}\n\n{CHECKPOINT_HEADER}\n\n{cp_md}\n"


def render_checkpoint_block(
    *,
    status: str,
    last_completed: str,
    next_action: str,
    repo_root: str,
    plan_path: str,
    plan_revision: str,
    workflow_run_id: str,
    operation_id: str,
    timestamp: str,
) -> str:
    return "\n".join(
        [
            CHECKPOINT_START,
            "- Checkpoint Version: 1",
            "- Workflow: pov-plan-only",
            f"- Status: {status}",
            f"- Last Completed: {last_completed}",
            f"- Next Action: {next_action}",
            f"- Repository: `{repo_root}`",
            f"- Active Plan: `{plan_path}`",
            f"- Plan Revision: sha256:{plan_revision}",
            f"- Workflow Run ID: {workflow_run_id}",
            f"- Operation ID: {operation_id}",
            f"- Timestamp: {timestamp}",
            CHECKPOINT_END,
        ]
    )


def locks_dir() -> str:
    override = os.environ.get("POVERELLO_LOCKS_DIR")
    if override:
        return override
    appdata = os.environ.get("APPDATA", "")
    if not appdata:
        fail("LOCKS_UNAVAILABLE", "APPDATA is not set; cannot locate Stable plan-locks directory")
    return os.path.join(appdata, "Poverello", "plan-locks")


class PlanLock:
    def __init__(self, abs_plan: str) -> None:
        ldir = locks_dir()
        os.makedirs(ldir, exist_ok=True)
        real = os.path.realpath(strip_verbatim(abs_plan))
        if os.name == "nt" and not real.startswith("\\\\?\\"):
            if real.startswith("\\\\"):
                real = "\\\\?\\UNC\\" + real[2:]
            else:
                real = "\\\\?\\" + real
        canonical = real.lower()
        lock_hash = hashlib.sha256(canonical.encode("utf-8")).hexdigest()[:32]
        self.lock_path = os.path.join(ldir, f"{lock_hash}.lock")
        self.fp = open(self.lock_path, "a+b")

    def __enter__(self) -> "PlanLock":
        if os.name == "nt":
            import msvcrt
            self.fp.seek(0)
            msvcrt.locking(self.fp.fileno(), msvcrt.LK_LOCK, 1)
        else:
            import fcntl
            fcntl.flock(self.fp.fileno(), fcntl.LOCK_EX)
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        try:
            if os.name == "nt":
                import msvcrt
                self.fp.seek(0)
                msvcrt.locking(self.fp.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                import fcntl
                fcntl.flock(self.fp.fileno(), fcntl.LOCK_UN)
        finally:
            self.fp.close()


plan_mutation_lock = PlanLock


def verify_locked_target_unchanged(initial_res: dict, locked_res: dict) -> None:
    if (
        os.path.normcase(os.path.realpath(locked_res["abs_plan_path"]))
        != os.path.normcase(os.path.realpath(initial_res["abs_plan_path"]))
        or locked_res["project_id"] != initial_res["project_id"]
        or locked_res["plan_path"] != initial_res["plan_path"]
    ):
        fail(
            "TARGET_CHANGED_DURING_LOCK",
            "Resolved plan target changed while acquiring PlanLock; refusing to update a different target",
        )


def atomic_cas_write_unlocked(abs_plan: str, repo_root: str, expected_sha256: str, final_text: str) -> str:
    check_components_inside_repo(repo_root, abs_plan)
    with open(abs_plan, "rb") as f:
        cur_sha = hashlib.sha256(f.read()).hexdigest()
    if cur_sha.lower() != expected_sha256.strip().lower():
        fail("CAS_CONFLICT", f"Concurrent modification detected before atomic replace ({cur_sha})")
    tmp_path = f"{abs_plan}.tmp.{os.getpid()}.{uuid.uuid4().hex[:8]}"
    with open(tmp_path, "w", encoding="utf-8", newline="\n") as f:
        f.write(final_text)
    try:
        # Pre-replace path/reparse and CAS revalidation immediately before os.replace
        check_components_inside_repo(repo_root, abs_plan)
        check_no_reparse(tmp_path)
        with open(abs_plan, "rb") as f:
            pre_replace_sha = hashlib.sha256(f.read()).hexdigest()
        if pre_replace_sha.lower() != expected_sha256.strip().lower():
            fail("CAS_CONFLICT", f"Concurrent modification detected immediately before atomic replace ({pre_replace_sha})")
        os.replace(tmp_path, abs_plan)
    except BaseException:
        if os.path.lexists(tmp_path):
            try:
                os.remove(tmp_path)
            except OSError:
                pass
        raise
    with open(abs_plan, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def public_resolve_dict(res: dict) -> dict:
    return {k: v for k, v in res.items() if not k.startswith("_")}


def cmd_resolve(args: argparse.Namespace) -> None:
    res = resolve_plan(args.repo, args.surface, args.plan)
    print(json.dumps(public_resolve_dict(res), indent=2, ensure_ascii=False))


def cmd_verify(args: argparse.Namespace) -> None:
    res = resolve_plan(args.repo, args.surface, args.plan)
    state = res["checkpoint_state"]
    if state == "MALFORMED_OR_DUPLICATE":
        fail("MALFORMED_MARKERS", "Plan contains duplicate or unbalanced checkpoint markers")
    if state == "NONE":
        fail("NO_CHECKPOINT", "No Poverello Execution Checkpoint found in plan; use `start` for a new workflow")
    if state == "DIFFERENT_WORKFLOW":
        fail(
            "WORKFLOW_MISMATCH",
            f"Existing checkpoint belongs to '{res['prior_workflow']}', not 'pov-plan-only'; use `start --archive-prior` to archive it and start a new workflow",
        )
    if state == "MISSING_PLAN_REVISION":
        fail(
            "MISSING_PLAN_REVISION",
            "Existing checkpoint lacks `Plan Revision` (legacy format); use `start --archive-prior` to archive it and start a new workflow",
        )
    if state == "HALTED":
        fail("WORKFLOW_HALTED", "Checkpoint Status is HALTED")
    if state == "MISSING_BINDING":
        fail("MISSING_CHECKPOINT_BINDING", res.get("binding_error", "Checkpoint identity binding is incomplete"))
    if state == "TARGET_MISMATCH":
        fail("CHECKPOINT_TARGET_MISMATCH", res.get("binding_error", "Checkpoint target does not match"))
    if state == "REVISION_MISMATCH":
        cp_rev = res["checkpoint"].get("Plan Revision", "").removeprefix("sha256:")
        fail(
            "REVISION_MISMATCH",
            f"Checkpoint Plan Revision ({cp_rev}) does not match current canonical revision ({res['canonical_plan_revision']})",
        )
    out = public_resolve_dict(res)
    out["verified"] = True
    print(json.dumps(out, indent=2, ensure_ascii=False))


def cmd_start(args: argparse.Namespace) -> None:
    if not args.workflow_run_id.strip() or not args.operation_id.strip():
        fail("INVALID_WORKFLOW_ID", "workflow-run-id and operation-id must be non-empty")
    initial_res = resolve_plan(args.repo, args.surface, args.plan)
    abs_plan = initial_res["abs_plan_path"]
    with PlanLock(abs_plan):
        res = resolve_plan(args.repo, args.surface, args.plan)
        verify_locked_target_unchanged(initial_res, res)
        if res["sha256_hash"].lower() != args.expected_sha256.strip().lower():
            fail(
                "CAS_CONFLICT",
                f"Expected sha256 {args.expected_sha256}, but file on disk has {res['sha256_hash']}",
            )
        state = res["checkpoint_state"]
        if state in ("MALFORMED_OR_DUPLICATE", "HALTED", "REVISION_MISMATCH", "RESUMABLE", "MISSING_BINDING", "TARGET_MISMATCH"):
            fail(
                f"START_BLOCKED_{state}",
                f"Cannot start new workflow from checkpoint_state='{state}' (use `verify` to resume a RESUMABLE workflow, or resolve malformed/halted/mismatched state first)",
            )
        if state in ("DIFFERENT_WORKFLOW", "MISSING_PLAN_REVISION") and not args.archive_prior:
            fail(
                "ARCHIVE_PRIOR_REQUIRED",
                f"Plan has prior checkpoint (state={state}, workflow={res['prior_workflow']}); pass `--archive-prior` to preserve it verbatim and start a new pov-plan-only baseline",
            )

        with open(abs_plan, "r", encoding="utf-8") as f:
            raw_text = f.read().replace("\r\n", "\n").replace("\r", "\n")
        if raw_text.startswith("\ufeff"):
            raw_text = raw_text[1:]

        now_iso = datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
        body_without_live_cp = strip_live_checkpoint(raw_text)

        if state in ("DIFFERENT_WORKFLOW", "MISSING_PLAN_REVISION"):
            raw_section = res["_raw_section"]
            if not raw_section:
                fail("ARCHIVE_FAILED", "Could not extract prior checkpoint section verbatim")
            fence = safe_fence_for(raw_section)
            prior_wf = res["prior_workflow"] or "unknown"
            archive_block = (
                f"### Archived Prior Checkpoint ({now_iso} / {prior_wf})\n\n"
                f"{fence}markdown\n{raw_section}\n{fence}"
            )
            audit_hdr = re.search(r"^##\s+(?:5\.\s+)?Execution History & Audit Trail\s*$", body_without_live_cp, re.MULTILINE)
            if audit_hdr:
                before_audit = body_without_live_cp[: audit_hdr.start()].rstrip("\n")
                from_audit = body_without_live_cp[audit_hdr.start() :].lstrip("\n")
                body_with_archive = f"{before_audit}\n\n{archive_block}\n\n{from_audit}"
            else:
                body_with_archive = body_without_live_cp.rstrip("\n") + "\n\n" + archive_block
            row = (
                f"| {now_iso} | START_WORKFLOW | {args.actor} | STARTED_FROM_PRIOR_CHECKPOINT | "
                f"Archived prior '{prior_wf}' checkpoint (has_plan_revision={str(res['has_plan_revision']).lower()}, "
                f"{describe_prior_checkpoint_binding(res, res['repo_root'], res['plan_path'])}); {args.notes} |"
            )
        else:
            body_with_archive = body_without_live_cp
            row = f"| {now_iso} | START_WORKFLOW | {args.actor} | STARTED | {args.notes} |"

        updated_body = append_audit_row(body_with_archive, row)
        new_rev = canonical_plan_revision(updated_body.encode("utf-8"))
        cp_md = render_checkpoint_block(
            status="IN_PROGRESS",
            last_completed="workflow_started",
            next_action="draft_plan",
            repo_root=res["repo_root"],
            plan_path=res["plan_path"],
            plan_revision=new_rev,
            workflow_run_id=args.workflow_run_id,
            operation_id=args.operation_id,
            timestamp=now_iso,
        )
        final_text = upsert_checkpoint(updated_body, cp_md)
        post_sha = atomic_cas_write_unlocked(abs_plan, res["repo_root"], args.expected_sha256, final_text)

    print(
        json.dumps(
            {
                "started": True,
                "archived_prior": state in ("DIFFERENT_WORKFLOW", "MISSING_PLAN_REVISION"),
                "prior_checkpoint_state": state,
                "prior_workflow": res["prior_workflow"],
                "project_id": res["project_id"],
                "plan_path": res["plan_path"],
                "sha256_hash": post_sha,
                "canonical_plan_revision": new_rev,
                "workflow_run_id": args.workflow_run_id,
                "operation_id": args.operation_id,
            },
            indent=2,
            ensure_ascii=False,
        )
    )


def cmd_commit(args: argparse.Namespace) -> None:
    if not args.workflow_run_id.strip() or not args.operation_id.strip():
        fail("INVALID_WORKFLOW_ID", "workflow-run-id and operation-id must be non-empty")
    initial_res = resolve_plan(args.repo, args.surface, args.plan)
    abs_plan = initial_res["abs_plan_path"]
    with PlanLock(abs_plan):
        res = resolve_plan(args.repo, args.surface, args.plan)
        verify_locked_target_unchanged(initial_res, res)
        if res["sha256_hash"].lower() != args.expected_sha256.strip().lower():
            fail(
                "CAS_CONFLICT",
                f"Expected sha256 {args.expected_sha256}, but file on disk has {res['sha256_hash']}",
            )
        state = res["checkpoint_state"]
        if state != "RESUMABLE":
            fail(
                f"COMMIT_BLOCKED_{state}",
                f"`commit` requires an active RESUMABLE pov-plan-only workflow (current state={state}); run `start` or `verify` first",
            )
        checkpoint_run_id = res["checkpoint"].get("Workflow Run ID", "").strip()
        if not checkpoint_run_id or args.workflow_run_id.strip() != checkpoint_run_id:
            fail(
                "WORKFLOW_RUN_MISMATCH",
                "commit workflow-run-id must match the active checkpoint run",
            )

        with open(abs_plan, "r", encoding="utf-8") as f:
            auth_text = f.read().replace("\r\n", "\n").replace("\r", "\n")
        if auth_text.startswith("\ufeff"):
            auth_text = auth_text[1:]

        if args.body_file:
            with open(args.body_file, "r", encoding="utf-8") as f:
                cand_text = f.read().replace("\r\n", "\n").replace("\r", "\n")
            if cand_text.startswith("\ufeff"):
                cand_text = cand_text[1:]
            clean_body = preserve_authoritative_history_in_candidate(auth_text, cand_text)
        else:
            clean_body = strip_live_checkpoint(auth_text)
        now_iso = datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
        row = f"| {now_iso} | {args.phase} | {args.actor} | {args.result} | {args.notes} |"
        updated_body = append_audit_row(clean_body, row)
        new_rev = canonical_plan_revision(updated_body.encode("utf-8"))

        cp_md = render_checkpoint_block(
            status=args.status,
            last_completed=args.last_completed,
            next_action=args.next_action,
            repo_root=res["repo_root"],
            plan_path=res["plan_path"],
            plan_revision=new_rev,
            workflow_run_id=args.workflow_run_id,
            operation_id=args.operation_id,
            timestamp=now_iso,
        )
        final_text = upsert_checkpoint(updated_body, cp_md)
        post_sha = atomic_cas_write_unlocked(abs_plan, res["repo_root"], args.expected_sha256, final_text)

    print(
        json.dumps(
            {
                "committed": True,
                "project_id": res["project_id"],
                "plan_path": res["plan_path"],
                "sha256_hash": post_sha,
                "canonical_plan_revision": new_rev,
                "operation_id": args.operation_id,
            },
            indent=2,
            ensure_ascii=False,
        )
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd", required=True)

    for name in ("resolve", "verify"):
        p = sub.add_parser(name)
        p.add_argument("--repo", default=os.getcwd())
        p.add_argument("--surface", default="codex")
        p.add_argument("--plan", default=None)

    ps = sub.add_parser("start")
    ps.add_argument("--repo", default=os.getcwd())
    ps.add_argument("--surface", default="codex")
    ps.add_argument("--plan", default=None)
    ps.add_argument("--expected-sha256", required=True)
    ps.add_argument("--actor", required=True)
    ps.add_argument("--notes", required=True)
    ps.add_argument("--workflow-run-id", required=True)
    ps.add_argument("--operation-id", required=True)
    ps.add_argument("--archive-prior", action="store_true")

    pc = sub.add_parser("commit")
    pc.add_argument("--repo", default=os.getcwd())
    pc.add_argument("--surface", default="codex")
    pc.add_argument("--plan", default=None)
    pc.add_argument("--expected-sha256", required=True)
    pc.add_argument("--phase", required=True)
    pc.add_argument("--actor", required=True)
    pc.add_argument("--result", required=True)
    pc.add_argument("--notes", required=True)
    pc.add_argument("--status", required=True)
    pc.add_argument("--last-completed", required=True)
    pc.add_argument("--next-action", required=True)
    pc.add_argument("--workflow-run-id", required=True)
    pc.add_argument("--operation-id", required=True)
    pc.add_argument("--body-file", default=None)

    args = parser.parse_args()
    if args.cmd == "resolve":
        cmd_resolve(args)
    elif args.cmd == "verify":
        cmd_verify(args)
    elif args.cmd == "start":
        cmd_start(args)
    elif args.cmd == "commit":
        cmd_commit(args)


if __name__ == "__main__":
    main()
