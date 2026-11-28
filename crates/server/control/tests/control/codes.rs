//! Every refusal has a code of the catalog, with a fix.

use build::BuildId;
use control::{Handle, HoldReason, Id, Refusal, Reservation, Spent};
use jiff::Timestamp;
use model::{Hash, Manifest, Route};

#[test]
fn every_refusal_is_worded_by_the_catalog() {
    let catalog = diag::Catalog::read().unwrap();
    let handle: Handle = "cstef".parse().unwrap();
    let deploy = Manifest::new(Vec::new()).unwrap().id();
    let (site, account) = (Id::from_bytes([1; 16]), Id::from_bytes([2; 16]));
    let (route, blob, build) = (Route::parse("/a").unwrap(), Hash::of("a"), BuildId::from_bytes([3; 16]));
    let until = Some(Timestamp::from_second(1).unwrap());
    let refusals = [
        Refusal::Reserved { handle: handle.clone(), reservation: Reservation::Infrastructure },
        Refusal::Reserved { handle: handle.clone(), reservation: Reservation::Brand },
        Refusal::Taken { handle: handle.clone() },
        Refusal::Held { handle: handle.clone(), why: HoldReason::Removed, until },
        Refusal::Held { handle, why: HoldReason::Verified, until: None },
        Refusal::Exists { id: "x".to_owned() },
        Refusal::NoAccount { account },
        Refusal::NoSite { site },
        Refusal::Removing { site },
        Refusal::MissingBlob { route, blob },
        Refusal::NoDeploy { site, deploy },
        Refusal::NotLive { site },
        Refusal::AlreadyLive { site, deploy },
        Refusal::NoCredential { credential: Id::from_bytes([4; 16]) },
        Refusal::NoManifest { deploy },
        Refusal::NoBuild { site, build },
        Refusal::MissingLog { build, blob },
        Refusal::Over { spent: Spent::Stored, value: 2, limit: 1 },
        Refusal::Over { spent: Spent::Builds, value: 2, limit: 1 },
        Refusal::Over { spent: Spent::Sites, value: 2, limit: 1 },
        Refusal::Suspended { site, kind: control::AbuseKind::Spam },
        Refusal::Serving { site },
        Refusal::Unstorable,
    ];
    for refusal in refusals {
        let entry = catalog.diagnostic.get(refusal.code()).unwrap_or_else(|| panic!("{} has no entry", refusal.code()));
        assert!(entry.fix.is_some(), "{} has no fix", refusal.code());
    }
}
