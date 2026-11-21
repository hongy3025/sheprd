# Antigravity's plan usage, in its usage panel.

## Setup: how to find each value the provider reads. Setting names,
## environment variables, URLs, cookie and header names, and the
## service's own menu paths stay as written.

usage-antigravity-setting-cli-path = The agy executable on the probed host when it is not on the PATH (install it with `brew install --cask antigravity-cli`, run `agy` once, and sign in). agy runs only when this is set or antigravity is in show_providers, as a signed-out agy opens a browser to sign in.
usage-antigravity-setting-proxy = Optional local-only proxy URL for `agy` usage commands on this machine (`http://` or `https://`, with optional percent-encoded `user:password@`). Leave unset to keep `HTTP_PROXY`, `HTTPS_PROXY`, and `NO_PROXY` from the environment; inherited `NO_PROXY` rules remain untouched. Not propagated to SSH or WSL hosts.

## The report: each model group's limits, as agy reports them. Model
## families (Gemini, Claude/GPT) and the names agy gives groups and limits
## stay as written.

# A group of models agy gives no name.
usage-antigravity-quota = Quota
# A limit's name: $group is the model group, such as Gemini; $cadence says
# whether the limit resets every five hours, weekly, or otherwise, when
# $name is agy's own name for it (empty for the other two).
usage-antigravity-window = { $group } { $cadence ->
        [session] 5-hour
        [weekly] weekly
       *[other] { $name }
    }
