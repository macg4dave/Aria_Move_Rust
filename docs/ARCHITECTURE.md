# Architecture

The binary loads strict XML configuration, applies CLI overrides, installs
logging and cancellation, validates the bases, resolves one source, then moves it.
Configuration errors fail before moving data. A missing config creates a template
except in dry-run mode. No source is chosen automatically.

`config` owns parsing, locations and base validation. `cli` borrows source paths
without converting them to Unicode. `fs_ops::resolve` checks links before
canonicalization and promotes nested files to their top-level download folder.

`fs_ops::mover` provides one lifecycle for files and directory trees:

1. Reject links, special files, overlapping paths and unfinished downloads,
   including adjacent aria2 control files.
2. Lock the source parent and completed base in canonical path order, then
   revalidate. Disabling locks is an explicit operational choice.
3. Compare ordered inventories across a short pause; reject a changing source.
4. Attempt an exclusive rename. Only a cross-device error selects copying.
5. Copy into a unique staging directory, verify bytes and source snapshots,
   apply the metadata policy, and publish without replacing an existing name.
6. Recheck the source inventory, then check each entry immediately before
   deleting it. Delete directories non-recursively, deepest first.

`transfer` owns copying, byte verification and identity/size/mtime snapshots;
`naming` owns collision names and propagates inspection errors; `space` owns the
advisory free-space preflight; `metadata` owns best-effort metadata preservation.
These remain separate because each has a distinct policy and focused tests.
No compatibility implementation or parallel file/tree mover is retained.

`platform` owns shared filesystem policy and delegates only native operations to
`unix` or `windows`. Linux uses exclusive rename flags and inode locks; Windows
uses native no-replace moves, file sharing restrictions and lock sidecars.
Logging uses a typed level threshold; no regex filter parser is needed.

Before publication, errors keep sources and discard the owned staging directory
where filesystem permissions permit cleanup. After publication, the destination
remains complete; cancellation or cleanup errors may leave some source entries.
Retrying can therefore create another complete destination with a collision name.
Metadata preservation is best effort, and extended attributes require `xattrs`.

Locks coordinate cooperating movers, not download clients or hostile filesystem
writers. Inventory sampling is a completion heuristic. Path-based checks have
race windows between inspection and filesystem operations; callers must control
the source and destination directories. Byte verification and identity checks do
not provide a filesystem snapshot of concurrently modified data.
