# Known risks

`d2r-manager` is a personal technical study for a trusted, single-user
workstation. It is not a hardened local service or a general save editor.

## Threat model

- Trust every local caller of the CLI and HTTP API.
- Trust every save directory, Git repository, hook, filter, and systemd unit.
- Do not expose the HTTP port through a proxy, container bridge, or network bind.
- Do not use this tool on a shared machine with untrusted local users.

The server binds only to `127.0.0.1` and checks `Host` and `Origin`. These checks
limit remote browser access, but they do not authenticate a local caller.

## Caller-controlled operations

HTTP requests can select save directories, backup repositories, lock files, and
the systemd user unit used by `apply`. Operations run with the server process
permissions.

Git commands can execute repository hooks or filters. Use repositories that you
created and control.

Save entries are read and written through their filesystem paths. Do not place
symbolic links inside a save directory. A descendant `.d2s` or `.d2i` symlink
can redirect an `apply` write outside the repository.

Use absolute local paths with `sync`. `rsync` can interpret some relative paths
containing a colon as remote-shell paths.

The HTTP body limit is 1 MiB. Save size, directory size, operation count, and
processing time have no equivalent hard limits. A local caller can block the
single request worker.

## Data integrity

Never run a write workflow while `D2R.exe` runs. The process check covers the
known executable name and command pattern, not every possible launcher setup.

`sync` takes a file lock, but `apply` does not take that lock. Stop the backup
timer and confirm that no backup process remains before `apply`.

`apply` pauses the selected systemd timer before it writes. A backup that already
started can still race with the transaction.

Save files are written one after another without atomic replacement. A process
crash, power loss, or later write failure can leave a partial result.

`apply` creates a Git checkpoint before the first write. After an interrupted
operation, inspect the repository and restore that checkpoint before further use.

The command writes only to a save mirror inside the selected Git repository. It
does not restore files into the live game directory.

## Compatibility limits

- Only save format 105 is accepted.
- Formats 98 and 99 must be converted by the game before this tool reads them.
- Transactions move or duplicate items, but they do not move gold.
- Online, ladder, and multiplayer save workflows are outside the project scope.

## Reports

GitHub Issues are open for reproducible findings. This personal project has no
support schedule or response-time commitment.
