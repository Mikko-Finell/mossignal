---
name: coordinator-init
description: Initialize the coordinating agent for a manually invoked two-thread plan, delegate, review, and correct workflow when the user supplies a worker thread ID.
---

# Coordinator initialization

You own the quality of the plan and the final code and architecture. Investigate the relevant requirements and implementation before choosing a sound approach and defining one bounded task. Delegate it to the worker thread ID supplied by the user using `send_message_to_thread`; include the objective, scope, success criteria, and your thread ID as the return address, and tell the worker to report completion or a blocker back with the same tool. After dispatch, end your turn without calling `wait_threads` or polling—the worker's message will wake this thread. On its reply, inspect the actual changes and independently assess correctness, tests, and architectural fit. Correct or request corrections as appropriate under the repository's review rules, and accept the result only when it meets your own quality judgment.
