#!/usr/bin/env bash
# Writes a dependency-light bearer-token verifier into an existing Go module
# and runs its tests.
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

DIR=""
PKG=""
while [ $# -gt 0 ]; do
    case "$1" in
        --dir) DIR="$2"; shift 2 ;;
        --pkg) PKG="$2"; shift 2 ;;
        *) die "unknown argument: $1" ;;
    esac
done
[ -n "$DIR" ] || die "--dir is required"
mkdir -p "$DIR"
PKG=${PKG:-$(basename "$(cd "$DIR" && pwd)" | tr -cs 'A-Za-z0-9_' '_' | sed 's/^_//; s/_$//')}
[ -n "$PKG" ] || die "--pkg is required when the directory name is not a valid identifier"

module_root="$DIR"
while [ "$module_root" != "/" ] && [ ! -f "$module_root/go.mod" ]; do
    module_root=$(dirname "$module_root")
done
[ -f "$module_root/go.mod" ] || die "no go.mod found above $DIR; run 'go mod init <module>' first"

version=${YUFU_SDK_VERSION:-}
if [ -z "$version" ]; then
    load_versions
    version="$YUFU_SDK_VERSION"
fi
[ -n "$version" ] || die "cannot determine the SDK version; set YUFU_SDK_VERSION"

render() {
    local template="$1" target="$2"
    python3 - "$template" "$target" "$PKG" <<'PY'
import sys

template = open(sys.argv[1], encoding="utf-8").read().replace("{{PKG}}", sys.argv[3])
open(sys.argv[2], "w", encoding="utf-8").write(template)
PY
}

render "$YUFU_SKILL_DIR/assets/go-resource/yufu_auth.go.tmpl" "$DIR/yufu_auth.go"
render "$YUFU_SKILL_DIR/assets/go-resource/yufu_auth_test.go.tmpl" "$DIR/yufu_auth_test.go"
log_ok "wrote $DIR/yufu_auth.go and $DIR/yufu_auth_test.go (package $PKG)"

(cd "$module_root" && go mod edit -require="github.com/Bin-hy/yufu-sdk-go@$version")
(cd "$module_root" && go mod tidy)
log_info "running go test in $DIR"
(cd "$DIR" && go test ./...)
log_ok "bearer verifier tests passed against SDK $version"