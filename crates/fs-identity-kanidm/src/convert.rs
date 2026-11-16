//! Directory group <-> Kanidm entry.
//!
//! Pattern: Adapter — the one place that maps our vocabulary (name, members, nested) to Kanidm's
//! attributes (`name`, `description`, `member`) and back. Kanidm vocabulary stops here.

use std::collections::BTreeMap;

use fs_groups_core::DirectoryGroup;

use crate::client::Entry;
use crate::error::{Error, Result};
use crate::naming::{group_id, group_name};
use crate::people::PersonMap;

/// What `read` returns for a member Kanidm has that we cannot map to a person: the raw value after
/// this prefix. `read` shows it; `apply` reports it and leaves it alone (ADR 0035 addendum).
pub const FOREIGN: &str = "kanidm:";

/// The raw values (as Kanidm answered them) of the members of a read group that nobody mapped.
pub fn foreign_accounts(group: &DirectoryGroup) -> Vec<String> {
    group
        .members
        .iter()
        .filter_map(|m| m.strip_prefix(FOREIGN))
        .map(str::to_string)
        .collect()
}

/// The attributes to send for a group. Fails, before anything is written, if a person has no account.
pub fn to_attrs(
    group: &DirectoryGroup,
    people: &dyn PersonMap,
) -> Result<BTreeMap<String, Vec<String>>> {
    let (mut accounts, mut missing) = (vec![], vec![]);
    for person in &group.members {
        if let Some(raw) = person.strip_prefix(FOREIGN) {
            accounts.push(raw.to_string()); // as read from Kanidm: passed through unchanged
            continue;
        }
        match people.account_of(person) {
            Some(a) => accounts.push(a),
            None => missing.push(person.clone()),
        }
    }
    if !missing.is_empty() {
        return Err(Error::UnmappedPeople(missing));
    }
    accounts.extend(group.nested.iter().map(|id| group_name(*id)));
    let mut attrs = BTreeMap::new();
    attrs.insert("name".to_string(), vec![group_name(group.id)]);
    attrs.insert("description".to_string(), vec![group.name.clone()]);
    attrs.insert("member".to_string(), accounts);
    Ok(attrs)
}

/// What to add to and remove from `member` so that our own members and nested groups match `wanted`.
/// Members nobody mapped are in neither list, so a request never names a value that is not ours.
/// Fails, before anything is written, if a wanted person has no account.
pub fn member_delta(
    entry: &Entry,
    wanted: &DirectoryGroup,
    people: &dyn PersonMap,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut add = vec![];
    let (mut have_people, mut have_nested) = (vec![], vec![]);
    let mut remove = vec![];
    for value in entry.values("member") {
        if let Some(child) = group_id(value) {
            if wanted.nested.contains(&child) {
                have_nested.push(child);
            } else {
                remove.push(value.clone());
            }
        } else if let Some(person) = people.person_of(value) {
            if wanted.members.contains(&person) {
                have_people.push(person);
            } else {
                remove.push(value.clone());
            }
        }
    }
    let mut missing = vec![];
    for person in wanted.members.iter().filter(|m| !m.starts_with(FOREIGN)) {
        if have_people.contains(person) {
            continue;
        }
        match people.account_of(person) {
            Some(a) => add.push(a),
            None => missing.push(person.clone()),
        }
    }
    if !missing.is_empty() {
        return Err(Error::UnmappedPeople(missing));
    }
    add.extend(
        wanted
            .nested
            .iter()
            .filter(|id| !have_nested.contains(id))
            .map(|id| group_name(*id)),
    );
    Ok((add, remove))
}

/// Read a Kanidm entry as a directory group; `None` if it is not one of ours (name without our prefix).
pub fn from_entry(entry: &Entry, people: &dyn PersonMap) -> Option<DirectoryGroup> {
    let id = group_id(entry.first("name").or_else(|| entry.first("spn"))?)?;
    let mut group = DirectoryGroup {
        id,
        name: entry.first("description").unwrap_or_default().to_string(),
        members: vec![],
        nested: vec![],
    };
    for value in entry.values("member") {
        if let Some(child) = group_id(value) {
            group.nested.push(child);
        } else if let Some(person) = people.person_of(value) {
            group.members.push(person);
        } else {
            group.members.push(format!("{FOREIGN}{value}"));
        }
    }
    Some(group.normalised())
}
