//! Members are written as changes only; the account map is case-blind; read-back foreign values pass.

mod common;

use common::{FakeKanidm, dg, person};
use fs_groups_core::GroupDirectory;
use fs_identity_kanidm::{FOREIGN, Method, PersonMap, StaticPersonMap, naming::group_name};
use fs_objects::ObjectId;

#[tokio::test]
async fn a_change_names_only_our_values_and_never_replaces_the_list() {
    let k = FakeKanidm::new();
    let d = k.directory();
    let id = ObjectId::new();
    d.apply(&dg(id, "Club", &["ada"], &[])).await.unwrap();
    k.groups.lock().unwrap()[0]
        .members
        .push("stranger@idm.test".into());
    k.clear_log();
    d.apply(&dg(id, "Club", &["bo"], &[])).await.unwrap();
    let writes: Vec<_> = k
        .log
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.method != Method::Get)
        .cloned()
        .collect();
    assert_eq!(
        writes.iter().map(|r| r.method).collect::<Vec<_>>(),
        [Method::Delete, Method::Post],
        "no PUT of the whole list"
    );
    for r in &writes {
        assert!(!format!("{:?}", r.body).contains("stranger"), "{r:?}");
    }
    assert_eq!(
        k.stored(&group_name(id)).unwrap().members,
        ["stranger@idm.test", "bo@idm.test"]
    );
}

#[tokio::test]
async fn findings_inside_one_group_are_sorted() {
    let k = FakeKanidm::new();
    let d = k.directory();
    let id = ObjectId::new();
    let want = dg(id, "Club", &["ada"], &[]);
    d.apply(&want).await.unwrap();
    k.groups.lock().unwrap()[0]
        .members
        .extend(["zed@idm.test".into(), "stranger@idm.test".into()]);
    let accounts: Vec<_> = d
        .apply_report(&want, false)
        .await
        .unwrap()
        .findings
        .into_iter()
        .map(|f| f.account)
        .collect();
    assert_eq!(accounts, ["stranger@idm.test", "zed@idm.test"]);
}

#[tokio::test]
async fn a_foreign_value_read_back_is_passed_on_unchanged() {
    let k = FakeKanidm::new();
    let d = k.directory();
    let id = ObjectId::new();
    d.apply(&dg(id, "Club", &["ada"], &[])).await.unwrap();
    k.groups.lock().unwrap()[0]
        .members
        .push("stranger@idm.test".into());
    let mut held = d.read(id).await.unwrap().unwrap();
    assert!(
        held.members
            .contains(&format!("{FOREIGN}stranger@idm.test"))
    );
    k.clear_log();
    let report = d.apply_report(&held, false).await.unwrap();
    assert_eq!(
        (report.changes.len(), report.findings.len(), k.writes()),
        (0, 1, 0)
    );
    held.members.push(format!("{FOREIGN}zed@idm.test"));
    d.apply(&dg(ObjectId::new(), "New", &[], &[]))
        .await
        .unwrap();
    let fresh = ObjectId::new();
    held.id = fresh;
    d.apply(&held).await.unwrap();
    let mut got = k.stored(&group_name(fresh)).unwrap().members;
    got.sort();
    assert_eq!(got, ["ada@idm.test", "stranger@idm.test", "zed@idm.test"]);
}

#[test]
fn accounts_are_trimmed_and_lower_cased_and_found_that_way() {
    let m = StaticPersonMap::new().with(person("ada"), "  Ada@IDM.test ");
    assert_eq!(
        m.account_of(&person("ada")).as_deref(),
        Some("ada@idm.test")
    );
    assert_eq!(m.person_of("ADA@idm.test"), Some(person("ada")));
    assert_eq!(m.person_of(" ada@idm.test"), Some(person("ada")));
}
