#!/bin/sh
# Apply the reviewed browser-owned pointer routing hook to a pinned Chromium src checkout.
# Usage: sh browser/engine/tools/apply-native-pointer-routing.sh /path/to/oya-electron/src
set -eu
engine_src=${1:?Pass the pinned Chromium src checkout}
patch_dir=$(CDPATH= cd -- "$(dirname -- "$0")/../patches" && pwd)
patch_file="$patch_dir/native-pointer-routing.patch"
if git -C "$engine_src" apply --reverse --check "$patch_file" 2>/dev/null; then
  printf '%s\n' 'Native pointer routing hook already applied.'
  exit 0
fi
git -C "$engine_src" apply --check "$patch_file"
git -C "$engine_src" apply "$patch_file"
printf '%s\n' 'Applied native pointer routing hook; build electron with no more than four compile jobs.'
