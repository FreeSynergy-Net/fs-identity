# Changelog

## Unreleased

* Changed: `apply` no longer removes Kanidm members that nobody mapped (`kanidm:<value>`). It reports them and leaves them in place;
  the comparison ignores them and a write carries them along unchanged (ADR 0035 addendum, ADR 0082).
* Added: `KanidmDirectory::apply_report(group, dry_run)` returning a `Report` (`changes`, sorted `findings`); types `Report`, `Finding`,
  `Reason`, `Change`. `GroupDirectory::apply` keeps its signature and calls it.
* Added: `convert::foreign_accounts` (internal) and tests in `tests/unmapped.rs`.
