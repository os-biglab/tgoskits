# Planner

Turn the task into a staged plan.

Include:

- scope
- affected layers
- implementation order
- verification commands
- rollback or fallback path
- known risks

Prefer the repository's existing validation commands and keep the plan minimal and testable.

When commands are likely to fail on the first attempt, include a debug loop strategy:

- what to inspect first when command fails
- what files to modify first
- what command to rerun after each fix
- stop condition for success

End with a compact checklist that other stages can execute directly.
