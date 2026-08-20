#!/usr/bin/env bash
set -euo pipefail

destination="${1:?usage: fetch-jars.sh <destination-dir>}"

fetch() {
    local url="$1"
    local name="$2"
    local expected="$3"
    local target="$destination/$name"

    if [ -f "$target" ] && [ "$(sha256sum "$target" | cut -d ' ' -f 1)" = "$expected" ]; then
        return 0
    fi

    mkdir -p "$destination"
    curl --fail --silent --show-error --location --output "$target.part" "$url"

    local actual
    actual="$(sha256sum "$target.part" | cut -d ' ' -f 1)"
    if [ "$actual" != "$expected" ]; then
        rm -f "$target.part"
        printf 'sha256 mismatch for %s\n  expected %s\n  measured %s\n' "$name" "$expected" "$actual" >&2
        exit 1
    fi

    mv "$target.part" "$target"
}

fetch \
    "https://repo1.maven.org/maven2/io/github/paladijn/d2rsavegameparser/3.2.0/d2rsavegameparser-3.2.0.jar" \
    "d2rsavegameparser-3.2.0.jar" \
    "0b2164f3c8219e390c42ccba3e2377dce2514561e1c32f5d0c1d4c9622960f89"

fetch \
    "https://repo1.maven.org/maven2/org/slf4j/slf4j-api/2.0.17/slf4j-api-2.0.17.jar" \
    "slf4j-api-2.0.17.jar" \
    "7b751d952061954d5abfed7181c1f645d336091b679891591d63329c622eb832"
