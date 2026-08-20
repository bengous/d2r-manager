root := justfile_directory()
oracle := root / "build/dotnet/bin/GoldensCsharp/release/goldens-csharp"

default:
    @just --list

check:
    cd {{ root }}/fixtures && sha256sum -c sha256sums.txt

build-oracles:
    dotnet build {{ root }}/tools/goldens-csharp/GoldensCsharp.csproj -c Release \
        --artifacts-path {{ root }}/build/dotnet --nologo -v quiet
    {{ root }}/tools/goldens-java/fetch-jars.sh {{ root }}/build/jars

goldens: build-oracles
    #!/usr/bin/env bash
    set -euo pipefail
    mapfile -t corpus < <(sed 's|^[0-9a-f]\{64\}  \./||' "{{ root }}/fixtures/sha256sums.txt" | sort)
    rm -rf "{{ root }}/goldens/csharp" "{{ root }}/goldens/java"
    "{{ oracle }}" dump "{{ root }}/fixtures" "{{ root }}/goldens/csharp" "${corpus[@]}"
    cd "{{ root }}/reference/d2rsavegameparser"
    java -Dslf4j.internal.verbosity=ERROR --class-path "{{ root }}/build/jars/*" \
        "{{ root }}/tools/goldens-java/JavaOracleDumper.java" \
        "{{ root }}/fixtures" "{{ root }}/goldens/java" "${corpus[@]}"
    cd "{{ root }}"
    "{{ oracle }}" report "{{ root }}/goldens/csharp" "{{ root }}/goldens/java" \
        "{{ root }}/goldens/REPORT.md" "${corpus[@]}"
