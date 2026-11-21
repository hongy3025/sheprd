# Codex's plan usage, in its usage panel.

## Setup: how to find each value the provider reads. Setting names,
## environment variables, URLs, and schemes stay as written.

usage-codex-setting-proxy = Optional local-only proxy URL for Codex usage requests on this machine (`http://` or `https://`, with optional percent-encoded `user:password@`). Leave unset to keep `HTTP_PROXY`, `HTTPS_PROXY`, and `NO_PROXY` from the environment; inherited `NO_PROXY` rules still apply. Not used on SSH or WSL hosts.

## The report: the code review limit and the account's credits. Plan
## names stay as ChatGPT sends them.

# The limit on Codex's code reviews.
usage-codex-code-review = Code review
# Credits that reset a reached limit early; the fact is how many are left.
usage-codex-reset-credits = Limit reset credits
usage-codex-available = Available
usage-codex-credits = Credits
usage-codex-balance = Balance
# A credit balance without limit.
usage-codex-unlimited = Unlimited
