# Coding-agent workspace

Language for coordinating coding sessions and preparing reviews.

## Language

**Input focus**:
Whether keyboard and paste input control dashboard navigation or the attached
native session. Returning to dashboard focus leaves the session running.

**Session history**:
The retained readable record of a session's prompts, agent responses, and
captured terminal output. It remains useful even when the original conversation
cannot be resumed.

**Native resume**:
Continuing an existing conversation through the original agent tool's supported
recovery mechanism. Reading session history or starting a new conversation is
not native resume.

**Review draft**:
A local, editable collection of review suggestions, human comments, and choices
associated with the code examined. A draft is distinct from a published review.

**Review comparison**:
The changes between the identified PR head and base/comparison revisions used
for review. It identifies the code being evaluated independently of current
local working changes.
