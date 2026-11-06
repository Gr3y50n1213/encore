# Requests between tracks

A track never edits another track's folder. When it needs something it does not own, it writes a request here and keeps working on something else.

**To ask:** copy `TEMPLATE.md` to `<your-track>-<number>-<short-title>.md` (tracks: `core`, `apple`, `android`, `windows-linux`; numbers count up per track, for example `apple-0003-queue-snapshot.md`). Fill in every field. Commit it. Stop work on that feature, or stub it inside your own folder and mark the stub `TODO(request <file name>)`.

**To answer:** the core track (or the owner, for a shared document or a decision) fills in the Answer, sets the status and commits. A request is closed when the thing it asked for is in `main` and `docs/core/CORE-API.md` or the shared document says so.

**What belongs here:** a missing core call or event, a change to a core type, a design token that is missing, a wireframe that disagrees with the plan, a decision only the owner can make.

**What does not:** anything inside your own folder, and questions Claude can answer by reading the plan.

Requests are the one place every track may add files. Never edit another track's request, except the track it is addressed to, which fills in the Answer.
