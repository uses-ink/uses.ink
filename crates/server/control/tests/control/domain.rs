//! Custom domains in the control plane alone: the claim is a row with a token, proving needs the
//! token among what the authority read, and every change is an event of the site.

use control::{DomainName, Found, NewClaim, Proving, Refusal, Token};

use crate::world::{World, at};

fn name(text: &str) -> DomainName {
    text.parse().unwrap()
}

#[test]
fn a_claim_is_listed_with_its_token_until_it_is_proved() {
    let world = World::new();
    let site = world.site("tea");
    let token = Token::from_bytes([5; 16]);
    let new = NewClaim { site: site.id, name: name("Tea.Example.com."), token, at: at(10) };
    let claim = world.run(world.control.claim_domain(new)).unwrap();
    assert_eq!(claim.name.as_str(), "tea.example.com");
    let listed = world.run(world.control.domains(site.id)).unwrap();
    assert_eq!((listed.proven.len(), listed.pending.len()), (0, 1));
    assert_eq!(listed.pending[0].token, token);
    assert_eq!(world.events("domain.claimed"), 1);

    let proving = |tokens: Vec<Token>, n| Proving {
        site: site.id,
        name: name("tea.example.com"),
        found: Found::new(tokens.len() as u64, tokens),
        at: at(n),
    };
    let wrong = world.run(world.control.prove_domain(proving(vec![Token::from_bytes([6; 16])], 20))).unwrap_err();
    assert_eq!(wrong.refusal(), Some(&Refusal::Unproven { name: name("tea.example.com"), records: 1 }));
    assert_eq!(world.events("domain.proven"), 0);
    // the right token among others is enough
    let proved = world.run(world.control.prove_domain(proving(vec![Token::from_bytes([6; 16]), token], 21))).unwrap();
    assert_eq!(proved.at, at(21));
    let listed = world.run(world.control.domains(site.id)).unwrap();
    assert_eq!((listed.proven.len(), listed.pending.len()), (1, 0));
    assert_eq!(world.events("domain.proven"), 1);
    // proving again answers the row it has and writes nothing
    let again = world.run(world.control.prove_domain(proving(vec![], 22))).unwrap();
    assert_eq!(again, proved);
    assert_eq!(world.events("domain.proven"), 1);
    world.run(world.control.remove_domain(site.id, &name("tea.example.com"), at(30))).unwrap();
    assert_eq!(world.events("domain.removed"), 1);
}

#[test]
fn a_site_that_does_not_exist_takes_no_claim() {
    let world = World::new();
    let ghost = control::Id::from_bytes([99; 16]);
    let new = NewClaim { site: ghost, name: name("a.example.com"), token: Token::from_bytes([1; 16]), at: at(1) };
    let refused = world.run(world.control.claim_domain(new)).unwrap_err();
    assert_eq!(refused.refusal(), Some(&Refusal::NoSite { site: ghost }));
}
