//! What `apply` did (or would do) and what it found but left alone (ADR 0035 addendum, ADR 0082).
//!
//! Pattern: Value objects — plain data the caller can print, store or test. Nothing here talks to Kanidm.

use std::fmt;

use fs_objects::ObjectId;

/// Why a member is reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    /// Kanidm has this member on the group, but nobody mapped the account to a person or role.
    NotMapped,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotMapped => f.write_str("not mapped"),
        }
    }
}

/// A member found in Kanidm that fs-groups does not manage. It is reported and left in place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// Our id of the group the account is in.
    pub group: ObjectId,
    /// The account as Kanidm answered it (an SPN or name).
    pub account: String,
    /// Why it is reported.
    pub reason: Reason,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "group {}: account {} ({}); left as it is",
            self.group, self.account, self.reason
        )
    }
}

/// One write `apply` makes (or would make).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// The group does not exist yet.
    Create,
    /// The display name (`description`) differs.
    SetDescription,
    /// Our own members or nested groups differ; unmapped members are carried along unchanged.
    SetMembers,
}

/// The outcome of one `apply` (or of a dry run).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Writes made, or, in a dry run, the writes that would be made.
    pub changes: Vec<Change>,
    /// Members left alone, sorted by group and account, without duplicates.
    pub findings: Vec<Finding>,
}

impl Report {
    /// Sort findings so the output is the same on every run.
    pub(crate) fn finish(mut self) -> Self {
        self.findings.sort_by(|a, b| {
            (a.group.to_string(), &a.account).cmp(&(b.group.to_string(), &b.account))
        });
        self.findings.dedup();
        self
    }

    /// Put reports of several groups together, findings sorted again.
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.changes.extend(other.changes);
        self.findings.extend(other.findings);
        self.finish()
    }
}
