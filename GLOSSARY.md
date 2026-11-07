# Coding-agent workspace

Language for coordinating coding sessions and preparing reviews.

## Language

**Project**:
A registered local Git repository together with its workspaces and worktrees.

**Workspace**:
One identifiable stream of coding work in a project, with its own working files,
branch, agent sessions, and retained history. It persists across conversations.
_Avoid_: Task (earlier documents use this for the same concept), checkout when
referring to the whole workspace.

**Worktree**:
The Git checkout and branch containing a workspace's working files.

**Agent session**:
A conversation and interactive runtime using a supported coding agent in a
workspace's worktree. In user-facing explanations, a chat is the conversation
within that session.

**Working diff**:
A comparison showing a workspace's current uncommitted changes, distinguishing staged,
unstaged, and new files.

**Archive checkpoint**:
A local commit saving a workspace's unfinished code changes when it is archived.
It remains a recovery point independently of the visible branch name, not a claim
that the work is complete or pushed remotely.

**Whole workspace comparison**:
A comparison showing a workspace branch's changes relative to its base, including
committed and current working changes. Earlier requirements call this the
whole-task comparison.

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
