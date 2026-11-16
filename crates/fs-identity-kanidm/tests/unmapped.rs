//! Members Kanidm has that nobody mapped: reported, never removed (ADR 0035 addendum, ADR 0082).

mod common;

use common::{FakeKanidm, dg};
use fs_groups_core::GroupDirectory;
use fs_identity_kanidm::{Change, FOREIGN, Finding, Reason, Report, naming::group_name};
use fs_objects::ObjectId;

#[tokio::test]
async fn an_account_nobody_mapped_shows_up_as_foreign_is_reported_and_stays() {
    let k = FakeKanidm::new();
    let d = k.directory();
    let id = ObjectId::new();
    let want = dg(id, "Club", &["ada"], &[]);
    d.apply(&want).await.unwrap();
    k.groups.lock().unwrap()[0]
        .members
        .push("stranger@idm.test".into());
    let seen = d.read(id).await.unwrap().unwrap();
    assert!(
        seen.members
            .contains(&format!("{FOREIGN}stranger@idm.test"))
    );
    k.clear_log();
    let report = d.apply_report(&want, false).await.unwrap();
    assert_eq!(
        k.stored(&group_name(id)).unwrap().members,
        ["ada@idm.test", "stranger@idm.test"],
        "the unmapped member is not removed"
    );
    assert_eq!(report.changes, []);
    assert_eq!(report.findings, [finding(id, "stranger@idm.test")]);
    assert_eq!(
        report.findings[0].to_string(),
        format!("group {id}: account stranger@idm.test (not mapped); left as it is")
    );
    assert_eq!(k.writes(), 0, "nothing own differs, so nothing is written");
}

#[tokio::test]
async fn replacing_our_members_carries_the_unmapped_ones_along() {
    let k = FakeKanidm::new();
    let d = k.directory();
    let id = ObjectId::new();
    d.apply(&dg(id, "Club", &["ada"], &[])).await.unwrap();
    k.groups.lock().unwrap()[0]
        .members
        .push("stranger@idm.test".into());
    let report = d
        .apply_report(&dg(id, "Club", &["bo", "cy"], &[]), false)
        .await
        .unwrap();
    assert_eq!(report.changes, [Change::SetMembers]);
    assert_eq!(report.findings, [finding(id, "stranger@idm.test")]);
    let mut members = k.stored(&group_name(id)).unwrap().members;
    members.sort();
    assert_eq!(members, ["bo@idm.test", "cy@idm.test", "stranger@idm.test"]);
}

#[tokio::test]
async fn an_account_in_several_groups_is_reported_once_per_group_sorted() {
    let k = FakeKanidm::new();
    let d = k.directory();
    let (a, b) = (ObjectId::new(), ObjectId::new());
    let (wa, wb) = (dg(a, "A", &["ada"], &[]), dg(b, "B", &["bo"], &[]));
    d.apply(&wa).await.unwrap();
    d.apply(&wb).await.unwrap();
    for g in k.groups.lock().unwrap().iter_mut() {
        g.members.push("zed@idm.test".into());
        g.members.push("stranger@idm.test".into());
    }
    let mut report = d.apply_report(&wa, false).await.unwrap();
    report = report.merge(d.apply_report(&wb, false).await.unwrap());
    let mut want = vec![
        finding(a, "stranger@idm.test"),
        finding(a, "zed@idm.test"),
        finding(b, "stranger@idm.test"),
        finding(b, "zed@idm.test"),
    ];
    want.sort_by_key(|f| (f.group.to_string(), f.account.clone()));
    assert_eq!(report.findings, want);
    assert_eq!(
        k.stored(&group_name(a)).unwrap().members.len(),
        3,
        "all kept"
    );
    assert_eq!(k.stored(&group_name(b)).unwrap().members.len(), 3);
}

#[tokio::test]
async fn without_unmapped_members_the_report_has_no_findings() {
    let k = FakeKanidm::new();
    let d = k.directory();
    let id = ObjectId::new();
    let want = dg(id, "Club", &["ada"], &[]);
    assert_eq!(
        d.apply_report(&want, false).await.unwrap().changes,
        [Change::Create]
    );
    let again = d.apply_report(&want, false).await.unwrap();
    assert_eq!(again, Report::default());
}

#[tokio::test]
async fn a_dry_run_reports_changes_and_findings_but_writes_nothing() {
    let k = FakeKanidm::new();
    let d = k.directory();
    let id = ObjectId::new();
    let created = d
        .apply_report(&dg(id, "Club", &["ada"], &[]), true)
        .await
        .unwrap();
    assert_eq!(created.changes, [Change::Create]);
    assert_eq!(k.writes(), 0);
    assert!(k.stored(&group_name(id)).is_none());

    d.apply(&dg(id, "Club", &["ada"], &[])).await.unwrap();
    k.groups.lock().unwrap()[0]
        .members
        .push("stranger@idm.test".into());
    k.clear_log();
    let dry = d
        .apply_report(&dg(id, "Renamed", &["bo"], &[]), true)
        .await
        .unwrap();
    assert_eq!(dry.changes, [Change::SetDescription, Change::SetMembers]);
    assert_eq!(dry.findings, [finding(id, "stranger@idm.test")]);
    assert_eq!(k.writes(), 0);
    assert_eq!(
        k.stored(&group_name(id)).unwrap().members,
        ["ada@idm.test", "stranger@idm.test"]
    );
}

fn finding(group: ObjectId, account: &str) -> Finding {
    Finding {
        group,
        account: account.into(),
        reason: Reason::NotMapped,
    }
}
