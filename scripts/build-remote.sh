#!/usr/bin/env bash
# Build get-server-ip natively on the target devices over SSH and fetch the
# binaries into dist/<host>/get-server-ip.
#
# Usage: scripts/build-remote.sh HOST...
#
# Every device has rustup installed with its native toolchain, so each build
# runs with the device's own compiler against its own glibc — no
# cross-compilation (and no Zig) involved.
#
# Exits non-zero if any host failed.

set -u -o pipefail

if [ "$#" -eq 0 ]; then
    echo "usage: $0 HOST..." >&2
    exit 2
fi
hosts=("$@")

root="$(cd "$(dirname "$0")/.." && pwd)"
remote_dir="builds/get-server-ip"
failures=0

for host in "${hosts[@]}"; do
    echo "==> $host: syncing sources"
    if ! {
        ssh -o BatchMode=yes "philippe@$host" "mkdir -p $remote_dir" &&
            rsync -a --delete \
                --exclude .git --exclude target --exclude dist \
                --exclude .zig-cache --exclude zig-out --exclude .DS_Store \
                -e "ssh -o BatchMode=yes" \
                "$root/" "philippe@$host:$remote_dir/"
    }; then
        echo "!! $host: sync failed" >&2
        failures=$((failures + 1))
        continue
    fi

    echo "==> $host: checking disk space"
    free_kb=$(ssh -o BatchMode=yes "philippe@$host" \
        "df -kP / | awk 'NR==2 {print \$4}'")
    if [ -z "$free_kb" ] || [ "$free_kb" -lt 1048576 ]; then
        echo "!! $host: less than 1 GiB free on / (got ${free_kb:-?} KiB)" >&2
        failures=$((failures + 1))
        continue
    fi

    echo "==> $host: building (cargo build --release --locked)"
    if ! ssh -o BatchMode=yes "philippe@$host" \
        "export PATH=\"\$HOME/.cargo/bin:\$PATH\"; cd $remote_dir && cargo build --release --locked" \
        | tail -2; then
        echo "!! $host: build failed" >&2
        failures=$((failures + 1))
        continue
    fi

    echo "==> $host: fetching binary"
    mkdir -p "$root/dist/$host"
    if ! scp -q -o BatchMode=yes \
        "philippe@$host:$remote_dir/target/release/get-server-ip" \
        "$root/dist/$host/get-server-ip"; then
        echo "!! $host: fetch failed" >&2
        failures=$((failures + 1))
        continue
    fi

    echo "==> $host: done ($(du -h "$root/dist/$host/get-server-ip" | cut -f1))"
done

exit "$failures"
