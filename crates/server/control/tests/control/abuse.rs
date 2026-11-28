//! Abuse reports and suspension: a suspended site serves nothing, takes nothing, and comes back whole.

use build::{BuildId, Trigger};
use control::table::{Report as ReportTable, Suspension as SuspensionTable};
use control::{
    AbuseKind, Contact, Detail, NewDeploy, NewJob, NewReport, Refusal, ReportError, ReportKind, ReportState, Suspension,
};

use crate::world::{World, at};

fn report(world: &World, handle: &str, n: i64) -> NewReport {
    NewReport {
        id: world.id::<ReportKind>(),
        handle: handle.parse().unwrap(),
        kind: AbuseKind::Phishing,
        detail: Detail::try_from("asks for a bank login".to_owned()).unwrap(),
        contact: Some(Contact::try_from("someone@example.org".to_owned()).unwrap()),
        at: at(n),
    }
}

fn suspension(site: &control::Site, report: Option<control::Id<ReportKind>>, n: i64) -> Suspension {
    Suspension { site: site.id, kind: AbuseKind::Phishing, by: "operator".to_owned(), report, at: at(n) }
}

#[test]
fn a_report_is_recorded_open_and_listed_newest_first() {
    let world = World::new();
    let (first, second) = (report(&world, "cstef", 10), report(&world, "elsewhere", 20));
    world.run(world.control.report(first.clone())).unwrap();
    let kept = world.run(world.control.report(second.clone())).unwrap();
    assert_eq!((kept.state, kept.kind), (ReportState::Open, AbuseKind::Phishing));
    let listed = world.run(world.control.reports(10)).unwrap();
    assert_eq!(listed.iter().map(|r| r.id).collect::<Vec<_>>(), [second.id, first.id]);
    assert_eq!(listed[1].contact.as_ref().map(Contact::as_str), Some("someone@example.org"));
    assert_eq!(world.run(world.control.reports(1)).unwrap().len(), 1);
    // a report names a handle, not a site: nothing here says whether either is one
    assert_eq!(world.count(ReportTable::Table), 2);
}

#[test]
fn reports_stop_at_the_limit_over_every_source() {
    let world = World::new().under(World::BARE.reported(2));
    let (first, second, third) = (report(&world, "one", 10), report(&world, "two", 20), report(&world, "three", 30));
    world.run(world.control.report(first)).unwrap();
    world.run(world.control.report(second)).unwrap();
    let full = world.run(world.control.report(third)).unwrap_err();
    assert_eq!(full.refusal(), Some(&Refusal::ReportsFull { limit: 2 }));
    assert_eq!(full.refusal().map(Refusal::code), Some("abuse.reports.full"));
    assert_eq!(world.count(ReportTable::Table), 2, "a refused report stores nothing");
    assert_eq!(world.run(world.control.reports(10)).unwrap().len(), 2);
}

#[test]
fn an_actioned_report_stops_counting_toward_the_limit() {
    let world = World::new().under(World::BARE.reported(1));
    let site = world.site("cstef");
    let first = report(&world, "cstef", 10);
    world.run(world.control.report(first.clone())).unwrap();
    let full = world.run(world.control.report(report(&world, "cstef", 20))).unwrap_err();
    assert_eq!(full.refusal(), Some(&Refusal::ReportsFull { limit: 1 }));
    world.run(world.control.suspend(suspension(&site, Some(first.id), 30))).unwrap();
    world.run(world.control.report(report(&world, "cstef", 40))).unwrap();
    assert_eq!(world.count(ReportTable::Table), 2, "the actioned report is kept, and no longer counts");
}

#[test]
fn the_text_of_a_report_is_bounded() {
    let long = "x".repeat(Detail::LONGEST + 1);
    assert_eq!(
        Detail::try_from(long),
        Err(ReportError::Long { field: "detail", limit: Detail::LONGEST, found: Detail::LONGEST + 1 })
    );
    assert_eq!(Detail::try_from("  ".to_owned()), Err(ReportError::Empty { field: "detail" }));
    assert_eq!(Contact::try_from("a\u{7}b".to_owned()), Err(ReportError::Control { field: "contact" }));
    assert!(Detail::try_from("two\nlines".to_owned()).is_ok());
}

#[test]
fn a_suspended_site_serves_nothing_and_takes_no_changes() {
    let world = World::new();
    let site = world.site("cstef");
    let manifest = world.stage(&site, &[("/", "page")]);
    let new = NewDeploy { site: site.id, manifest: manifest.clone(), build: None, at: at(10) };
    world.run(world.control.deploy(new)).unwrap();
    assert!(world.run(world.control.serving(&site.handle)).unwrap().is_some());
    let filed = world.run(world.control.report(report(&world, "cstef", 15))).unwrap();
    world.run(world.control.suspend(suspension(&site, Some(filed.id), 20))).unwrap();
    // no deploy is served, and the notice is there to be answered with
    assert!(world.run(world.control.serving(&site.handle)).unwrap().is_none());
    let notice = world.run(world.control.suspended(&site.handle)).unwrap().unwrap();
    assert_eq!((notice.kind, notice.by.as_str(), notice.report), (AbuseKind::Phishing, "operator", Some(filed.id)));
    // the report is actioned in the same step, and the log says so
    assert_eq!(world.run(world.control.reports(10)).unwrap()[0].state, ReportState::Actioned);
    assert_eq!(world.events("site.suspended"), 1);
    // nothing is accepted for it
    let again = NewDeploy { site: site.id, manifest, build: None, at: at(30) };
    let refused = world.run(world.control.deploy(again)).unwrap_err();
    assert_eq!(refused.refusal(), Some(&Refusal::Suspended { site: site.id, kind: AbuseKind::Phishing }));
    assert_eq!(refused.refusal().map(Refusal::code), Some("site.state.suspended"));
    let job =
        NewJob { id: BuildId::from_bytes([1; 16]), site: site.id, trigger: Trigger::Push, dedupe: None, at: at(31) };
    let refused = world.run(world.control.request_build(job)).unwrap_err();
    assert!(matches!(refused.refusal(), Some(Refusal::Suspended { .. })), "{refused:?}");
    // a second suspension says it is already
    let twice = world.run(world.control.suspend(suspension(&site, None, 40))).unwrap_err();
    assert!(matches!(twice.refusal(), Some(Refusal::Suspended { .. })), "{twice:?}");
}

#[test]
fn reinstating_gives_back_what_was_there() {
    let world = World::new();
    let site = world.site("cstef");
    let manifest = world.stage(&site, &[("/", "page")]);
    let new = NewDeploy { site: site.id, manifest: manifest.clone(), build: None, at: at(10) };
    world.run(world.control.deploy(new)).unwrap();
    world.run(world.control.suspend(suspension(&site, None, 20))).unwrap();
    world.run(world.control.reinstate(site.id, at(30))).unwrap();
    let serving = world.run(world.control.serving(&site.handle)).unwrap().unwrap();
    assert_eq!(serving.live.deploy, manifest.id());
    assert!(world.run(world.control.suspended(&site.handle)).unwrap().is_none());
    assert_eq!(world.events("site.reinstated"), 1);
    let not = world.run(world.control.reinstate(site.id, at(40))).unwrap_err();
    assert_eq!(not.refusal(), Some(&Refusal::Serving { site: site.id }));
}

#[test]
fn removing_a_suspended_site_removes_its_suspension() {
    let world = World::new();
    let site = world.site("cstef");
    world.run(world.control.suspend(suspension(&site, None, 20))).unwrap();
    assert_eq!(world.count(SuspensionTable::Table), 1);
    world.run(world.control.remove_site(site.id, at(30))).unwrap();
    assert_eq!(world.count(SuspensionTable::Table), 0);
}
