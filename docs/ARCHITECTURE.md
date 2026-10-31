# Architecture

The binary loads strict XML configuration, applies CLI overrides, installs
logging and cancellation, validates the bases, resolves one source, then moves it.
Configuration errors fail before moving data. A missing config creates a template
except in dry-run mode. No source is chosen automatically.

`config` owns parsing, locations and base validation. `cli` borrows source paths
without converting them to Unicode. `fs_ops::resolve` checks links before
canonicalization. Explicit paths and single-file aria2 hooks select exactly that
entry. Multi-file hooks select the top-level download folder under download_base;
these downloads must each have their own folder. Use --source-path to select an
exact folder when using a different layout.

`fs_ops::mover` provides one lifecycle for files and directory trees:

1. Reject links, special files, overlapping paths and unfinished downloads,
   including adjacent aria2 control files.
2. Lock only the individual source download, then revalidate. Independent
   downloads sharing incoming/completed directories do not block each other.
   Disabling locks is an explicit operational choice.
3. Inventory the source once. The completion hook establishes completion; an
   arbitrary delay or a temporary-looking extension does not. Refuse aria2
   control files, but accept legitimate .tmp, .part and .crdownload data files.
4. Attempt an exclusive rename. Only a cross-device error selects copying.
5. Stream files sequentially within each hook into a unique staging directory.
   Independent hooks run concurrently, without per-hook CPU-sized worker pools.
   Verify bytes and source snapshots,
   apply the metadata policy, and publish without replacing an existing name.
   If another mover claims that name, choose a fresh name and retry publication
   of the already-verified copy; do not copy the data again.
6. Recheck the source inventory, then check each entry immediately before
   deleting it. Delete directories non-recursively, deepest first.

`transfer` owns copying, byte verification and identity/size/mtime snapshots;
`naming` owns collision names and propagates inspection errors; `space` owns the
advisory free-space preflight; `metadata` owns best-effort metadata preservation.
These remain separate because each has a distinct policy and focused tests.
No compatibility implementation or parallel file/tree mover is retained.

`platform` owns shared filesystem policy and delegates only native operations to
`unix` or `windows`. Linux uses exclusive rename flags and inode locks; Windows
uses native no-replace moves, file sharing restrictions and per-source lock
sidecars. A directory download is locked as one source, never by locking its
shared incoming parent or completed directory. Duplicate hooks for the same
source serialize and revalidate it after locking.
Logging uses a typed level threshold; no regex filter parser is needed.

Before publication, errors keep sources and discard the owned staging directory
where filesystem permissions permit cleanup. After publication, the destination
remains complete; cancellation or cleanup errors may leave some source entries.
Retrying can therefore create another complete destination with a collision name.
Metadata preservation is best effort, and extended attributes require `xattrs`.

Locks coordinate cooperating movers, not download clients or hostile filesystem
writers. A completion hook (or the manual caller) must supply a finished download. Path-based checks have
race windows between inspection and filesystem operations; callers must control
the source and destination directories. Byte verification and identity checks do
not provide a filesystem snapshot of concurrently modified data.

On Unix, unsupported directory fsync (EINVAL/ENOTSUP) is tolerated for mounted
filesystems. File-content flushes and verification remain mandatory; actual I/O,
space, permission and read-only errors still fail. Crash durability of directory
updates depends on the destination filesystem. Windows lock sidecars are created
exclusively: an existing ordinary file is never opened with delete-on-close.
