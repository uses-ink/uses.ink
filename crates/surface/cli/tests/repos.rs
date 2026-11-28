//! A site's repository through the real binary (decisions 0160 to 0162): `ink repo claim|prove|
//! rotate|list|release` and the Settings forms against a fake forge that speaks https with a
//! certificate authority made here, then a signed webhook that builds the site, a replay that
//! does not, and a rotation that ends the old secret. No network is touched.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "tests stop at the first surprise"
)]

#[path = "../../../core/https/tests/support/forge.rs"]
mod forge;
mod keyed;
mod rig;

use forge::{Authority, Forge, Reply};
use rig::{Browser, Machine, Server};
use serde_json::{Value, json};

const NAME: &str = "forge.test";
const PROJECT: &str = "/api/v4/projects/me%2Fblog";
const FILE: &str = "/api/v4/projects/me%2Fblog/repository/files/.well-known%2Fuses-ink/raw?ref=main";

/// Adding sites over the API.
struct Sites;

impl Sites {
    /// Adds the site `journal` built from `url`, signed as the operator, asking again while the
    /// server is still booting or a connection is dropped. Answers its id.
    fn add(server: &Server, url: &str) -> String {
        let key = keyed::Keyed::operator(&server.data, &server.audience);
        let body = json!({ "handle": "journal", "source": { "kind": "git", "url": url } }).to_string();
        let started = std::time::Instant::now();
        let added = loop {
            let auth = key.sign("POST", "/v1/sites", body.as_bytes());
            let headers = [("Authorization", auth.as_str()), ("Content-Type", "application/json")];
            let got = server.once(("POST", "/v1/sites"), &headers, &body);
            let booting = got.text.is_empty() || got.text.contains("auth.server.restarting");
            if !booting || started.elapsed() > Server::PATIENCE {
                break got;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        };
        assert_eq!(added.status(), "201", "{}", added.text);
        added.json()["id"].as_str().unwrap().to_owned()
    }
}

/// A server that reaches a fake forge, with the site `journal` built from a repository on it.
struct Fixture {
    server: Server,
    forge: Forge,
    site: String,
    _authority: Authority,
}

impl Fixture {
    fn start(name: &str) -> Self {
        let authority = Authority::new();
        let forge = Forge::start(&authority, &[NAME]);
        let pem = std::env::temp_dir().join(format!("{name}-{}.pem", forge.address().port()));
        std::fs::write(&pem, authority.pem()).unwrap();
        forge.reply("GET", PROJECT, Reply::Status(200, br#"{"default_branch":"main"}"#.to_vec()));
        forge.reply("GET", FILE, Reply::Status(404, b"{}".to_vec()));
        let route = format!("{NAME}={}", forge.address());
        let server = Server::start(
            name,
            &[
                "--site-ttl",
                "0",
                "--forge-hooks",
                "--forge-gitlab",
                NAME,
                "--forge-route",
                &route,
                "--git-ca",
                pem.to_str().unwrap(),
                "--poll-lookout",
                "60",
            ],
        );
        let site = Sites::add(&server, &format!("https://{NAME}/me/blog"));
        Self { server, forge, site, _authority: authority }
    }

    /// A machine signed in as the operator.
    fn operator(&self) -> Machine<'_> {
        let mut machine = self.server.machine("operator");
        machine.dir = self.server.root.join("site");
        std::fs::create_dir_all(&machine.dir).unwrap();
        let data = self.server.data.to_str().unwrap().to_owned();
        let logged = machine.ink(&["login", "--data", &data, "--label", "operator"]);
        assert_eq!(logged.code, 0, "{}", logged.said());
        machine
    }

    /// The owner's browser, signed in with the first-run link.
    fn browser(&self) -> Browser<'_> {
        let link = self.server.links().pop().unwrap();
        let path = link[link.find("/login/").unwrap()..].to_owned();
        let mut browser = self.server.browser();
        let entered = browser.send(("POST", &path), "");
        assert_eq!(entered.status(), "303", "{}", entered.text);
        browser.keep(&entered);
        browser
    }

    /// The forge shows the challenge in the file.
    fn publish(&self, challenge: &str) {
        self.forge.reply("GET", FILE, Reply::Status(200, format!("# the repo\n{challenge}\n").into_bytes()));
    }

    /// A push event of the project, as GitLab sends it, with `token` and delivery `id`.
    fn push(&self, token: &str, id: &str, after: &str) -> rig::Got {
        let body = json!({
            "object_kind": "push",
            "ref": "refs/heads/main",
            "after": after,
            "project": { "path_with_namespace": "me/blog", "web_url": format!("https://{NAME}/me/blog"), "default_branch": "main" },
        })
        .to_string();
        let headers = [
            ("X-Gitlab-Token", token),
            ("X-Gitlab-Event", "Push Hook"),
            ("X-Gitlab-Event-UUID", id),
            ("Content-Type", "application/json"),
        ];
        self.server.http(("POST", "/v1/hooks/gitlab"), &headers, &body)
    }

    /// The triggers of the site's builds, newest first.
    fn triggers(&self) -> Vec<String> {
        let key = keyed::Keyed::operator(&self.server.data, &self.server.audience);
        let target = format!("/v1/sites/{}/builds", self.site);
        let auth = key.sign("GET", &target, b"");
        let got = self.server.http(("GET", &target), &[("Authorization", auth.as_str())], "");
        assert_eq!(got.status(), "200", "{}", got.text);
        let items: Vec<Value> = got.json()["items"].as_array().unwrap().clone();
        items.iter().map(|item| item["trigger"].as_str().unwrap().to_owned()).collect()
    }
}

const SHA: &str = "6dcb09b5b57875f334f61aebed695e2e4193db5e";
const OTHER: &str = "0123456789abcdef0123456789abcdef01234567";

#[test]
fn a_repository_is_claimed_proved_and_builds_the_site_on_a_signed_push() {
    let fixture = Fixture::start("repos-cli");
    let machine = fixture.operator();
    let site = fixture.site.as_str();

    let claimed = machine.ink(&["repo", "claim", "--site", site, "--json"]);
    assert_eq!(claimed.code, 0, "{}", claimed.said());
    let json = claimed.json();
    let secret = json["secret"].as_str().unwrap().to_owned();
    let challenge = json["challenge"].as_str().unwrap().to_owned();
    assert_eq!((json["repo"].as_str(), json["state"].as_str()), (Some("forge.test/me/blog"), Some("pending")));
    assert_eq!((json["forge"].as_str(), json["hook"].as_str()), (Some("gitlab"), Some("/v1/hooks/gitlab")));
    assert_eq!(secret.len(), 64);
    let plain = machine.ink(&["repo", "claim", "--site", site]);
    assert!(plain.out.contains("shown once"), "{}", plain.said());
    // the second claim made a new challenge and a new secret; the first secret is gone
    let secret = {
        let again = machine.ink(&["repo", "claim", "--site", site, "--json"]);
        assert_eq!(again.code, 0, "{}", again.said());
        again.json()["secret"].as_str().unwrap().to_owned()
    };
    let challenge = machine.ink(&["repo", "list", "--site", site, "--json"]).json()["repos"][0]["challenge"]
        .as_str()
        .map_or(challenge, str::to_owned);

    // a webhook before the proof builds nothing: the repository is no site's
    assert_eq!(fixture.push(&secret, "d-0", SHA).status(), "401");

    // the forge has no file yet
    let early = machine.ink(&["repo", "prove", "--site", site]);
    assert_ne!(early.code, 0, "{}", early.said());
    assert!(early.err.contains("repo.proof.missing") || early.err.contains("uses-ink"), "{}", early.err);
    fixture.publish("uses-ink-repo=00000000000000000000000000000000");
    assert_ne!(machine.ink(&["repo", "prove", "--site", site]).code, 0, "a file with another challenge proves nothing");

    fixture.publish(&challenge);
    let proved = machine.ink(&["repo", "prove", "--site", site, "--json"]);
    assert_eq!(proved.code, 0, "{}{}", proved.said(), fixture.server.said());
    assert_eq!(proved.json()["state"], "proven");
    assert!(machine.ink(&["repo", "list", "--site", site]).out.contains("building on push"));

    // a signed push queues one build with the trigger `push`
    let before = fixture.triggers().len();
    let taken = fixture.push(&secret, "d-1", SHA);
    assert_eq!(taken.status(), "202", "{}{}", taken.text, fixture.server.said());
    assert_eq!(taken.json()["outcome"], "queued");
    let triggers = fixture.triggers();
    assert_eq!(triggers.len(), before + 1, "{triggers:?}");
    assert_eq!(triggers[0], "push");
    // the same delivery again is refused, a new delivery of the same commit builds nothing more
    let replayed = fixture.push(&secret, "d-1", SHA);
    assert_eq!((replayed.status(), replayed.problem().as_str()), ("409", "hook.delivery.replayed"));
    assert_eq!(fixture.push(&secret, "d-2", SHA).json()["outcome"], "duplicate");
    assert_eq!(fixture.triggers().len(), before + 1);
    // a wrong token is one answer, whatever the reason
    assert_eq!(fixture.push("wrong", "d-3", OTHER).status(), "401");

    // rotation ends the old secret at once
    let rotated = machine.ink(&["repo", "rotate", "--site", site, "--json"]);
    assert_eq!(rotated.code, 0, "{}", rotated.said());
    let fresh = rotated.json()["secret"].as_str().unwrap().to_owned();
    assert_ne!(fresh, secret);
    assert_eq!(fixture.push(&secret, "d-4", OTHER).status(), "401");
    assert_eq!(fixture.push(&fresh, "d-5", OTHER).status(), "202");

    // release: the repository follows nothing, and the claim is gone
    let released = machine.ink(&["repo", "release", "--site", site]);
    assert_eq!(released.code, 0, "{}", released.said());
    assert_eq!(fixture.push(&fresh, "d-6", SHA).status(), "401");
    assert!(machine.ink(&["repo", "list", "--site", site]).out.contains("No repositories claimed."));
    assert_ne!(machine.ink(&["repo", "release", "--site", site]).code, 0);
}

#[test]
fn the_settings_screen_claims_proves_rotates_and_releases_and_shows_a_secret_once() {
    let fixture = Fixture::start("repos-web");
    let browser = fixture.browser();

    let empty = browser.send(("GET", "/journal/settings"), "");
    assert_eq!(empty.status(), "200", "{}", empty.text);
    assert!(empty.body().contains("action=\"/journal/repo-claim\""), "{}", empty.body());
    let foreign = browser.send_with(("POST", "/journal/repo-claim"), &[("Origin", "http://evil.example")], "");
    assert_eq!(foreign.status(), "403", "{}", foreign.text);

    let claimed = browser.send(("POST", "/journal/repo-claim"), "");
    assert_eq!(claimed.status(), "200", "{}{}", claimed.text, fixture.server.said());
    let page = claimed.body();
    assert!(page.contains("shown here once"), "{page}");
    let after = &page[page.find("webhook secret of").unwrap()..];
    let secret = after.split("<code>").nth(2).unwrap().split('<').next().unwrap().to_owned();
    assert_eq!(secret.len(), 64, "{after}");
    let challenge = page[page.find("uses-ink-repo=").unwrap()..].split('<').next().unwrap().to_owned();
    // the next view of the page does not have it
    let reload = browser.send(("GET", "/journal/settings"), "");
    assert!(!reload.body().contains(&secret), "the secret is on no other page");
    assert!(reload.body().contains("action=\"/journal/repo-prove\""), "{}", reload.body());

    let early = browser.send(("POST", "/journal/repo-prove"), "");
    assert_eq!(early.status(), "409", "{}", early.text);
    assert!(early.body().contains("repo.proof.missing"), "{}", early.body());
    fixture.publish(&challenge);
    let proved = browser.send(("POST", "/journal/repo-prove"), "");
    assert_eq!(proved.status(), "303", "{}{}", proved.text, fixture.server.said());
    assert!(browser.send(("GET", "/journal/settings"), "").body().contains("Builds the site on every push"));
    assert_eq!(fixture.push(&secret, "w-1", SHA).status(), "202");

    let rotated = browser.send(("POST", "/journal/repo-rotate"), "");
    assert_eq!(rotated.status(), "200", "{}", rotated.text);
    assert_eq!(fixture.push(&secret, "w-2", OTHER).status(), "401", "the rotated secret is gone");

    let released = browser.send(("POST", "/journal/repo-release"), "");
    assert_eq!(released.status(), "303", "{}", released.text);
    assert!(browser.send(("GET", "/journal/settings"), "").body().contains("No repository claimed"));
}

#[test]
fn a_server_that_was_not_asked_to_reach_forges_says_so_and_has_no_hook_route() {
    let server = Server::start("repos-off", &["--site-ttl", "0"]);
    let site = Sites::add(&server, "https://forge.test/me/blog");
    let mut machine = server.machine("operator");
    machine.dir = server.root.join("site");
    std::fs::create_dir_all(&machine.dir).unwrap();
    let data = server.data.to_str().unwrap().to_owned();
    assert_eq!(machine.ink(&["login", "--data", &data, "--label", "operator"]).code, 0);
    let claimed = machine.ink(&["repo", "claim", "--site", &site]);
    assert_ne!(claimed.code, 0, "{}", claimed.said());
    assert!(claimed.err.contains("repo.forges.off"), "{}", claimed.err);
    assert!(claimed.err.contains("--forge-hooks"), "{}", claimed.err);
    // the webhook path is no route when the flag is off
    let hook = server.http(("POST", "/v1/hooks/github"), &[("Content-Type", "application/json")], "{}");
    assert_eq!((hook.status(), hook.problem().as_str()), ("400", "auth.signature.malformed"), "{}", hook.text);
}
