//! [`KanidmDirectory`]: Kanidm as a `GroupDirectory` (ADR 0047).
//!
//! Pattern: Adapter (+ Strategy) — the groups core says what should be; this brings Kanidm there over
//! REST v1 through the HTTP seam. `apply` reads first and writes only what differs, so a repeat is
//! quiet and a hand edit of our own members in Kanidm is put back; members nobody mapped are reported, never removed. Cycles are refused in fs-groups, not here.

use std::sync::Arc;

use async_trait::async_trait;
use fs_groups_core::{DirectoryGroup, GroupDirectory, Result};
use fs_interfaces::Capabilities;
use fs_objects::ObjectId;

use crate::client::KanidmClient;
use crate::convert::{FOREIGN, foreign_accounts, from_entry, member_values, to_attrs};
use crate::http::HttpSeam;
use crate::naming::{PREFIX, group_name};
use crate::people::PersonMap;
use crate::report::{Change, Finding, Reason, Report};

/// Kanidm as a directory that mirrors our groups.
#[derive(Clone)]
pub struct KanidmDirectory {
    client: KanidmClient,
    people: Arc<dyn PersonMap>,
}

impl KanidmDirectory {
    /// A directory over a transport and a person map.
    pub fn new(seam: Arc<dyn HttpSeam>, people: Arc<dyn PersonMap>) -> Self {
        Self {
            client: KanidmClient::new(seam),
            people,
        }
    }
}

impl KanidmDirectory {
    /// Bring one group to the wanted state and say what was done and what was left alone.
    ///
    /// Only our own (mapped) members and nested groups are replaced. Members Kanidm has that nobody
    /// mapped are never removed or changed: they are carried along in the write and returned as
    /// findings (ADR 0035 addendum, ADR 0082). With `dry_run`, nothing is written; the report shows
    /// what would be.
    pub async fn apply_report(&self, group: &DirectoryGroup, dry_run: bool) -> Result<Report> {
        let wanted = group.clone().normalised();
        let name = group_name(wanted.id);
        let attrs = to_attrs(&wanted, self.people.as_ref())?; // fails before any write
        let mut report = Report::default();
        let Some(entry) = self.client.group(&name).await? else {
            report.changes.push(Change::Create);
            if !dry_run {
                self.client.create_group(&attrs).await?;
            }
            return Ok(report);
        };
        let Some(current) = from_entry(&entry, self.people.as_ref()) else {
            return Ok(report); // cannot happen: the name is ours; nothing to compare
        };
        let foreign = foreign_accounts(&current);
        report.findings = foreign
            .iter()
            .map(|account| Finding {
                group: wanted.id,
                account: account.clone(),
                reason: Reason::NotMapped,
            })
            .collect();
        if current.name != wanted.name {
            report.changes.push(Change::SetDescription);
            if !dry_run {
                self.client
                    .set_attr(&name, "description", std::slice::from_ref(&wanted.name))
                    .await?;
            }
        }
        // Compare only what is ours; unmapped members do not count as a difference.
        let mut own = current.clone();
        own.members.retain(|m| !m.starts_with(FOREIGN));
        if own.members != wanted.members || own.nested != wanted.nested {
            report.changes.push(Change::SetMembers);
            if !dry_run {
                let mut members = member_values(&wanted, self.people.as_ref())?;
                members.extend(foreign); // PUT replaces the attribute: send the unmapped ones back unchanged
                self.client.set_attr(&name, "member", &members).await?;
            }
        }
        Ok(report.finish())
    }
}

#[async_trait]
impl GroupDirectory for KanidmDirectory {
    fn capabilities(&self) -> Capabilities {
        let mut c = Capabilities::new("kanidm");
        c.supported("directory.apply").supported("directory.read");
        c.unsupported(
            "invite.create",
            "Kanidm has no invitations; fs-groups keeps them (kanidm#4659)",
        );
        c.unsupported(
            "right.grant",
            "Kanidm delegation is all or nothing; fs-groups keeps the finer rights (kanidm#4641)",
        );
        c
    }

    async fn read(&self, id: ObjectId) -> Result<Option<DirectoryGroup>> {
        let entry = self.client.group(&group_name(id)).await?;
        Ok(entry.and_then(|e| from_entry(&e, self.people.as_ref())))
    }

    async fn list(&self) -> Result<Vec<DirectoryGroup>> {
        let found = self.client.search_groups(PREFIX).await?;
        Ok(found
            .iter()
            .filter_map(|e| from_entry(e, self.people.as_ref()))
            .collect())
    }

    async fn apply(&self, group: &DirectoryGroup) -> Result<()> {
        self.apply_report(group, false).await?;
        Ok(())
    }

    async fn remove(&self, id: ObjectId) -> Result<()> {
        self.client.delete_group(&group_name(id)).await?;
        Ok(())
    }
}
