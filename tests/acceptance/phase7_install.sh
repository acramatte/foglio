#!/usr/bin/env bash
# Fresh same-host Ubuntu userspace installation, never a second-device claim.
set -euo pipefail
cd "$(dirname "$0")/../.."
package="${FOGLIO_DEB:-$PWD/target/release/bundle/deb/Foglio_0.1.0_amd64.deb}"
driver="$(command -v tauri-driver)"
test -f "$package"
image='ubuntu:26.04@sha256:513c074113a871b51a8d16ab445c88779d6452d937a164fb5cc479f32668a41d'
docker run --rm --init \
  --mount "type=bind,source=$package,target=/tmp/foglio.deb,readonly" \
  --mount "type=bind,source=$PWD/tests/acceptance,target=/workspace/tests/acceptance,readonly" \
  --mount "type=bind,source=$driver,target=/usr/local/bin/tauri-driver,readonly" \
  --env DEBIAN_FRONTEND=noninteractive \
  "$image" bash -euc '
    apt-get update
    apt-get install -y /tmp/foglio.deb python3 xvfb xauth webkitgtk-webdriver
    dpkg-query -W foglio libwebkit2gtk-4.1-0
    useradd --create-home foglio-test
    runuser -u foglio-test -- python3 -c "from pathlib import Path; p=Path.home()/\"library\"; p.mkdir(); (p/\"ordinary.md\").write_bytes(b\"# Ordinary\\n\\nFresh installation fixture\\n\")"
    library=/home/foglio-test/library
    before=$(sha256sum "$library/ordinary.md")
    runuser -u foglio-test -- notes --library "$library" reindex
    runuser -u foglio-test -- notes --library "$library" doctor
    runuser -u foglio-test -- notes --library "$library" search Ordinary
    for suite in phase4 phase5 phase6 phase7_keyboard; do
      runuser -u foglio-test -- env FOGLIO_DESKTOP_BINARY=/usr/bin/foglio-desktop python3 "/workspace/tests/acceptance/$suite.py"
    done
    # Delete only the disposable test user cache, then reconstruct.
    runuser -u foglio-test -- python3 -c "from pathlib import Path; import shutil; shutil.rmtree(Path.home()/\".cache/foglio\")"
    runuser -u foglio-test -- notes --library "$library" reindex
    runuser -u foglio-test -- notes --library "$library" doctor
    test "$before" = "$(sha256sum "$library/ordinary.md")"
    apt-get purge -y foglio
    test ! -e /usr/bin/notes
    test ! -e /usr/bin/foglio-desktop
    test "$before" = "$(sha256sum "$library/ordinary.md")"
    printf "%s\n" "PASS: fresh Ubuntu container install, shipped CLI/native suites, cache reconstruction and package purge preserve library" "LIMIT: same host/kernel and virtual display; no second-device or physical-display qualification"
  '
