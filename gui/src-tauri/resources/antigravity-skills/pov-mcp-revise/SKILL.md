---
name: pov-mcp-revise
description: Poverello MCP経由で、設定されたモデルへ処理を依頼するSkill
---

# Poverello MCP Revise

Revise an existing implementation plan based on user feedback using the Poverello external MCP planner.

## Instructions
1. Identify the implementation plan to revise using the following priority:
   - Priority 1: An implementation plan present in the active conversation context.
   - Priority 2: An active implementation plan artifact (e.g. `implementation_plan.md`).
   - If neither is found, STOP immediately and ask the user to provide or specify the plan to revise. Do not guess from historical transcripts.
2. Identify the user's requested revisions, feedback, or new constraints.
3. Inspect additional repository context only if needed.
4. Call `poverello/revise` exactly once with:
   - the original task;
   - the current implementation plan;
   - the requested revisions and feedback;
   - newly relevant constraints or repository context.
5. Produce the revised implementation plan, preserving valid sections of the previous plan unless the revisions require changing them.
6. Do not edit files.
7. Do not run implementation commands.
8. Do not begin implementation.
9. Stop after presenting the revised implementation plan.
