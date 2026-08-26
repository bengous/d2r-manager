# d2r-manager

A Rust case study for Diablo II: Resurrected offline save migration, inspection,
and transactional stash merges.

This repository turns one real Windows-to-Linux recovery into reproducible code.
It is a personal technical study, not a supported save editor or online-game tool.

## Case study: Windows-to-Linux recovery

The source machine held 49 offline Windows characters. The Linux installation
already held four characters. Across the collected saves, 22 used format 105,
27 used format 99, and four used format 98.

The recovery followed a conservative sequence:

1. Mount the Windows NTFS partition read-only.
2. Inspect headers, internal names, versions, and checksums before any write.
3. Resolve the `Cica` name collision by changing the file and internal name.
4. Copy the Windows characters while preserving the newer Linux `Elektra`.
5. Open 31 old characters in the game to convert formats 98 and 99 to 105.
6. Verify 53 characters, valid checksums, matching names, and no duplicates.
7. Merge two full format-105 shared stashes through the game.

The stash merge used a carrier character and temporary shared-stash swaps. The
game wrote the final reference file. Gold moved in the game because
`d2r-manager` transactions do not move gold.

`d2r-manager` was built immediately after that migration. It froze the real
corpus, specified format 105, and replayed the stash merge as 135 typed move
operations. The replay preserves every item, empties the Windows stash, and
matches the game-written Linux result.

This chronology matters. The project did not convert old save formats itself.
The game performed that conversion, then this project formalized and tested the
resulting format-105 workflow.

## What the project does

- Mirrors offline saves into a Git repository with process, path, and lock guards.
- Lists and filters items across characters and shared stashes.
- Compares two save directories by container and item fingerprint.
- Builds validated JSON plans for item moves or explicit duplications.
- Applies plans through a Git checkpoint, write, read-back, and commit sequence.
- Exposes the same tasks through a loopback-only HTTP API.

It does not create items, edit statistics, support online saves, or accept any
save format other than 105. It never writes directly into the live game save
directory.

## Architecture

The workspace has six one-way Rust crates:

```text
format <- domain <- app <- infra <- cli
                                <- server
```

- `format` reads and writes the v105 bitstream without I/O.
- `domain` models containers, grids, fingerprints, rules, and transaction states.
- `app` holds use cases and ports.
- `infra` implements files, Git, rsync, processes, systemd timers, and time.
- `cli` and `server` expose the application boundary.

The transaction model uses distinct `Plan`, `ValidatedPlan`, and `AppliedPlan`
states. Invalid operations stop before the first save write.

## Build and inspect

Clone the pinned reference repositories with the project:

```sh
git clone --recurse-submodules https://github.com/bengous/d2r-manager.git
cd d2r-manager
cargo build --release -p cli
target/release/d2r-manager --help
```

The pinned Rust toolchain is in `rust-toolchain.toml`. `rsync`, `pgrep`, Git,
and systemd user services are required only by the related operational commands.

The CLI provides these commands:

| Command | Purpose |
|---|---|
| `sync` | Mirror saves into a Git repository and commit changes |
| `ls` | List every item by container |
| `find` | Filter items by container, code, quality, or fingerprint |
| `diff` | Compare two save directories |
| `plan` | Validate operations and print normalized JSON without writing |
| `apply` | Apply a plan to a save mirror inside a Git repository |
| `serve` | Expose the same tasks on `127.0.0.1` |

Run `d2r-manager <command> --help` for the complete interface.

## Verification

```sh
just test
just check
```

The differential harness compares the Rust implementation with two pinned
oracles. D2SSharp produces the primary JSON model and byte-for-byte round-trip
verdict. d2rsavegameparser provides an independent Java cross-check.

The committed corpus contains 60 old personal save files. Goldens let normal
Rust tests run without .NET or Java. `just goldens` rebuilds them when the
oracles or corpus change.

## Known risks

Read [KNOWN_RISKS.md](KNOWN_RISKS.md) before any operational use. The HTTP API
has no authentication, request paths are trusted, and `apply` has known crash
and concurrency limits.

## Sources and credits

- [D2SSharp](https://github.com/ResurrectedTrader/D2SSharp), declared MIT, was
  the primary format reference and C# oracle.
- [halbu](https://github.com/feored/halbu), MIT, cross-checked header offsets.
- [d2rsavegameparser](https://github.com/Paladijn/d2rsavegameparser), LGPL-2.1,
  is executed as a Java oracle. Its source was not ported into the Rust code.
- The generated Rust tables retain only the game-table columns required to
  parse and validate format 105.

Diablo II: Resurrected and its game data belong to Blizzard Entertainment and
its licensors. This project is not affiliated with or endorsed by Blizzard.

## License

Original project code and documentation are licensed under the [MIT License](LICENSE).
Third-party submodules and game-derived data remain under their respective terms.
