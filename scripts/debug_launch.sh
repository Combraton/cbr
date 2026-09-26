#!/bin/sh
# The one provider launch a person may type, and the only one.
#
# docs/VERIFICATION.md rule 4 says the provider is launched on the owner's
# machine only through the test suite or an authorised run. That rule was
# written down, broken, restated, and broken again — so the fix is not a
# third restatement. It is this: the permitted path is now easier to type
# than the forbidden one, and it is the one that cannot reach a model.
#
# What it does:
#   * a throwaway data directory, a socket inside it, and a credential the
#     provider issues at start, all printed in the form `cbr` wants;
#   * a configuration this script writes itself, in the production
#     format, naming no `model_runtime` — and `--config` refused, so it
#     cannot be handed one that does;
#   * `--permit-model-network` and `--calibrate` refused by name, before
#     anything starts.
#
# It therefore reads no credential of the owner's, opens no socket to any
# provider, and spends nothing. A launch that needs to do any of those is
# an authorised run, planned before it, and is not this.
#
# Usage:  sh scripts/debug_launch.sh [--register-repository <id>=<path>]...
#
# Ctrl-C ends it, and so does its standard input ending: that is how the
# provider is stopped on this binding, so run it from a terminal rather
# than with `< /dev/null`, which ends it at once. The data directory is
# left behind on purpose, so what it wrote can be looked at; the line on
# standard error says how to remove it.

set -eu

here=$(cd "$(dirname "$0")/.." && pwd)

die() {
    printf 'debug_launch: %s\n' "$1" >&2
    exit 1
}

refuse() {
    printf 'debug_launch: %s\n' "$1" >&2
    printf 'debug_launch: this launch never calls a model. See VERIFICATION rule 4 (docs/VERIFICATION.md): a launch that reads a credential or opens a socket to a provider is the owner'"'"'s to run deliberately, and a refusal worth checking is worth a test.\n' >&2
    exit 1
}

# **The arguments, filtered down to the ones that cannot reach a model.**
# Anything unrecognised is refused rather than passed through: a
# pass-through is how `--config` would arrive.
n=$#
i=0
while [ "$i" -lt "$n" ]; do
    arg=$1
    shift
    i=$((i + 1))
    case $arg in
        --permit-model-network)
            refuse "--permit-model-network is not something this script will pass."
            ;;
        --calibrate)
            refuse "--calibrate makes live calls and is not something this script will pass."
            ;;
        --config)
            refuse "--config is refused: a configuration file is where a model_runtime would come from, so this launch writes its own and runs under that."
            ;;
        --register-repository)
            [ "$i" -lt "$n" ] || die "--register-repository needs <id>=<path>"
            value=$1
            shift
            i=$((i + 1))
            set -- "$@" --register-repository "$value"
            ;;
        *)
            die "unknown argument $arg; this script takes --register-repository <id>=<path> and nothing else"
            ;;
    esac
done

binary=${CBR_PROVIDER_BIN:-}
if [ -z "$binary" ]; then
    cargo build --manifest-path "$here/Cargo.toml" -p cbr-provider --locked >&2
    binary="$here/target/debug/cbr-provider"
fi
[ -x "$binary" ] || die "$binary is not an executable provider"

# 0700 from mktemp, which is what `socket::check_directory` insists on.
dir=$(mktemp -d "${TMPDIR:-/tmp}/cbr-debug.XXXXXX")

# **The configuration, written here and nowhere else.** It is in the
# production format, which refuses every conformance test control by
# name — including `model`, the fake transport — and it names no
# `model_runtime`, which is the member a credential read would come
# from. `--config` is refused above, so this file is the only one this
# launch can run under.
principal=caller
printf '{"format":"cbr-config/1","principal":"%s","authority_principals":["%s"]}\n' \
    "$principal" "$principal" > "$dir/cbr.json"

printf 'socket %s\n' "$dir/provider.sock"
printf 'credential %s\n' "$dir/data/credentials/$principal"
printf 'data %s\n' "$dir/data"
printf 'debug_launch: no model, no credential of yours, no network. Ctrl-C to stop; then rm -rf %s\n' "$dir" >&2

# **`exec`, so the process you stop is the provider.** A shell holding it
# as a child is a second process to reason about, and an orphaned provider
# over a data directory is exactly the state this script exists to avoid.
exec "$binary" --data-dir "$dir/data" --config "$dir/cbr.json" \
    --socket "$dir/provider.sock" "$@"
