---
name: pov-mcp-plan
description: Poverello MCP経由で、設定されたモデルへ処理を依頼するSkill
---

# Poverello MCP Plan

Create a comprehensive implementation plan for the requested task using the Poverello external MCP planner.

## Instructions
1. Inspect the repository and identify only the files and code relevant to the user's request.
2. Read the relevant implementation, configuration, and tests needed to understand the task.
3. Summarize only the repository context necessary for implementation planning.
4. Call `poverello/plan` exactly once with:
   - the user's task;
   - the relevant repository context;
   - important implementation constraints.
5. Use the returned external-model plan to produce the final comprehensive implementation plan.
6. Do not edit files.
7. Do not run implementation commands.
8. Do not begin implementation.
9. Stop after presenting the implementation plan.
