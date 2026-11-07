# Public repository checklist

This repository is public and the app is open source, so nothing in it is secret. The aim is that nothing in it can harm the owner, a user or a contributor. Check every item before each commit and push. `tools/ci/check_public.py` enforces the mechanical ones in CI, and the rest is review.

## Never commit

- **Personal details**: email addresses, real names (beyond a chosen public handle), phone numbers, home addresses, school or employer names.
- **Credentials**: API keys, access, refresh or Quick Connect tokens, passwords, private keys, certificates with keys, `.env` files, keystores, signing profiles.
- **This machine's details**: home-folder paths with a user name (`C:\Users\<name>`, `/Users/<name>`, `/home/<name>`), device serial numbers, local network addresses or host names of real machines (use 192.168.1.x or `example.org` placeholders), Wi-Fi names.
- **Real listening history and library data**: play logs, databases (`*.sqlite`), exports, `private-history/`. Recordings go through the anonymizer (`tools/fixture-recorder`). Commit only scrubbed shapes and summary numbers, and check the result before committing.
- **Media**: real album art, audio or lyrics. Fixtures use the project's own art, scrambled text and generated media.
- **Large files**: over 8 MB.

## Check in every change

1. `python3 -I -B tools/ci/check_public.py` passes (CI runs it too).
2. New fixtures: open them and read a sample. Names, IDs, paths, server names and user names are scrambled or hashed; tokens are `anonymized-token`.
3. Logs and errors never print a token, a stream URL (it can carry `ApiKey`) or a request body.
4. New dependencies: a reviewed licence (`tools/ci/check_licences.py`), an exact version, and a maintained source.
5. Workflows:
   - `permissions: contents: read` unless a job needs more;
   - actions pinned to a commit SHA;
   - no `pull_request_target`;
   - no `${{ github.event.* }}` text from a pull request inside a `run:` line (pass it through `env:`);
   - no secrets in jobs that run on pull requests.
6. Docs:
   - no machine paths or device identifiers;
   - private details of the owner's setup (their PC, phones, library size) only as far as a reviewer needs them;
   - no claim of a test that didn't run.
7. Commit identity: the GitHub `noreply` address, never a personal one (`git config user.email`).

## Before making a release or changing visibility

- Scan the whole history, not only the latest files:
  `git grep -P '<pattern>' $(git rev-list --all)`.
- GitHub settings:
  - secret scanning and push protection on;
  - Dependabot alerts on;
  - private vulnerability reporting on (SECURITY.md points to it);
  - "Keep my email addresses private" and "Block command line pushes that expose my email" on.
- A licence file is in place; without one the code is not open source.
