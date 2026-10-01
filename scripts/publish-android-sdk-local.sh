#!/usr/bin/env bash
set -euo pipefail

usage() {
    printf 'Usage: %s local\n' "$0" >&2
}

if [[ $# -ne 1 ]]; then
    usage
    exit 2
fi

sdk_version="$1"
if [[ "$sdk_version" != local ]]; then
    printf 'Invalid SDK version: %s\n' "$sdk_version" >&2
    printf '%s\n' 'This helper only installs the reserved local version.' >&2
    exit 2
fi

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

if [[ -n "${GRADLE_BIN:-}" ]]; then
    if [[ -x "$GRADLE_BIN" ]]; then
        gradle_cmd=("$GRADLE_BIN")
    elif command -v "$GRADLE_BIN" >/dev/null 2>&1; then
        gradle_cmd=("$GRADLE_BIN")
    else
        printf 'Gradle executable not found: %s\n' "$GRADLE_BIN" >&2
        exit 1
    fi
elif [[ -x android-sdk/gradlew ]]; then
    gradle_cmd=(./android-sdk/gradlew)
elif command -v gradle >/dev/null 2>&1; then
    gradle_cmd=(gradle)
else
    printf '%s\n' 'Gradle is required (set GRADLE_BIN to an installed Gradle or wrapper).' >&2
    exit 1
fi

bash ./scripts/build-android-sdk.sh
"${gradle_cmd[@]}" -p android-sdk publishToMavenLocal "-PsdkVersion=$sdk_version"

printf '\nInstalled com.cold04:inkreaderlink-uniffi:%s into mavenLocal().\n' "$sdk_version"
printf 'SDK source commit: %s\n' "$(git rev-parse HEAD)"
if ! git diff --quiet || ! git diff --cached --quiet || [[ -n "$(git ls-files --others --exclude-standard)" ]]; then
    printf '%s\n' 'The build also included uncommitted or untracked SDK source changes.'
fi
