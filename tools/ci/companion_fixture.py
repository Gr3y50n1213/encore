"""Make the replay fixture the companion server's Docker test uses (core.yml, the
server-image job): recorded.json plus a `GET /Users/Me` reply for its user.

    python3 -I -B tools/ci/companion_fixture.py RECORDED.json OUT.json

The cores in the test sign in through recorded.json, so the companion's sign-in
check must name that same user. The owner's real `/Users/Me` reply is in
recorded-phase4.json, but that recording was anonymized with its own key, so its
IDs differ. This answer is built from recorded.json's own sign-in reply (the same
user record Jellyfin returns for `/Users/Me`): a stand-in made from recorded data,
named as such in the Phase 4 review. Standard library only.
"""

import json
import sys


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    with open(sys.argv[1], encoding="utf-8") as f:
        exchanges = json.load(f)
    auth = next(
        x for x in exchanges if x["path"] == "/Users/AuthenticateWithQuickConnect"
    )
    me = {
        "origin": auth.get("origin"),
        "method": "GET",
        "path": "/Users/Me",
        "query": {},
        "status": 200,
        "body": auth["body"]["User"],
    }
    with open(sys.argv[2], "w", encoding="utf-8") as f:
        json.dump(exchanges + [me], f)
    print(f"{len(exchanges) + 1} exchanges, with /Users/Me for the recorded user")


if __name__ == "__main__":
    main()
