# PHPantom — Refactoring

Technical debt and internal cleanup tasks. This document is the first
item in every sprint. The sprint cannot begin feature work until this
gate is clear.

> **Housekeeping:** When a task is completed, remove it from this
> document entirely. Do not strike through or mark as done.

## Sprint-opening gate process

Every sprint lists "Clear refactoring gate" as its first item,
linking here. When an agent starts a sprint, follow these steps:

1. **Resolve outstanding items.** If this document contains any tasks,
   work through them. Remove each one as it is completed.
2. **Request a fresh session.** After completing refactoring work,
   stop and ask the user to start a new session. Analysis must happen
   in a session where no refactoring work was performed (since loading
   `AGENTS.md`). This ensures the analyst is not biased by the work
   just done.
3. **Analyze (fresh session only).** In a fresh session with no
   outstanding items, review the codebase for technical debt that
   would hinder the current sprint's tasks. Read the sprint items,
   scan the relevant modules, and decide whether any structural
   cleanup should happen first. If issues are found, add them to this
   document, work through them, and go back to step 2.
4. **Declare the gate clear.** When a fresh-session analysis finds no
   issues worth adding, remove the "Clear refactoring gate" row from
   the current sprint table. The sprint is now open for feature work.

A "fresh session" means one where no refactoring edits have been made
since the session started. The point is to get an unbiased second look
at the codebase after cleanup, not to rubber-stamp work just completed
in the same context.

### What belongs here

Only add items that would actively hinder the upcoming sprint's work
or that have accumulated enough friction to justify a focused cleanup
pass. Small fixes that can be done inline during feature work should
just be done inline. Items do not need to be scoped to the sprint's
feature area, but they should be completable in reasonable time (not
multi-week rewrites that would stall the sprint indefinitely).