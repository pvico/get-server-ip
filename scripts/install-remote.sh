#!/usr/bin/env bash
# Install the built binaries onto the target devices into ~/bin.
#
# Usage: scripts/install-remote.sh HOST...
#
# Requires the binaries to have been fetched by scripts/build-remote.sh into
# dist/<host>/get-server-ip. The install is atomic (uploaded to
# ~/bin/get-server-ip.new, then moved into place) and the config template is
# seeded to ~/bin/.get-server-ip/config.json only when the device has no
# configuration yet — an existing config is never overwritten.
#
# Exits non-zero if any host failed.

set -u -o pipefail

if [ "$#" -eq 0 ]; then
    echo "usage: $0 HOST..." >&2
    exit 2
fi
hosts=("$@")

root="$(cd "$(dirname "$0")/.." && pwd)"
bin_dir="bin"
failures=0

for host in "${hosts[@]}"; do
    binary="$root/dist/$host/get-server-ip"
    if [ ! -x "$binary" ]; then
        echo "!! $host: no binary at dist/$host/get-server-ip — run scripts/build-remote.sh $host first" >&2
        failures=$((failures + 1))
        continue
    fi

    echo "==> $host: installing to ~/$bin_dir"
    if ! {
        ssh -o BatchMode=yes "philippe@$host" "mkdir -p ~/$bin_dir" &&
            scp -q -o BatchMode=yes "$binary" "philippe@$host:$bin_dir/get-server-ip.new" &&
            ssh -o BatchMode=yes "philippe@$host" \
                "chmod +x ~/$bin_dir/get-server-ip.new && mv ~/$bin_dir/get-server-ip.new ~/$bin_dir/get-server-ip"
    }; then
        echo "!! $host: install failed" >&2
        failures=$((failures + 1))
        continue
    fi

    echo "==> $host: checking configuration"
    if ssh -o BatchMode=yes "philippe@$host" \
        "test -f ~/$bin_dir/.get-server-ip/config.json"; then
        echo "==> $host: config already present, left untouched"
    else
        if ! {
            ssh -o BatchMode=yes "philippe@$host" "mkdir -p ~/$bin_dir/.get-server-ip" &&
                scp -q -o BatchMode=yes "$root/config.json" \
                    "philippe@$host:$bin_dir/.get-server-ip/config.json"
        }; then
            echo "!! $host: config seed failed" >&2
            failures=$((failures + 1))
            continue
        fi
        echo "==> $host: config template installed (edit ~/$bin_dir/.get-server-ip/config.json)"
    fi

    echo "==> $host: verify"
    if ! ssh -o BatchMode=yes "philippe@$host" "~/$bin_dir/get-server-ip --version"; then
        echo "!! $host: installed binary does not run" >&2
        failures=$((failures + 1))
        continue
    fi

    echo "==> $host: done"
done

exit "$failures"
