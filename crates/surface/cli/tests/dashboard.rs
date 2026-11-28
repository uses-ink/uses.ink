//! The dashboard of `ink serve --data`, through the real binary and raw HTTP as a browser makes
//! it: sign in by the emailed link, see the site list, push a folder with `ink push` and see the
//! deploy and its build, end a session, sign out, and mint recovery codes. Every page has one
//! `h1` and labelled controls, and nothing that changes state accepts a request from another
//! origin.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "tests stop at the first surprise"
)]

mod rig;

use rig::{Browser, Got, Server};

/// A form body of one field.
fn field(name: &str, value: &str) -> String {
    let escaped: String = value
        .bytes()
        .map(|byte| match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => char::from(byte).to_string(),
            other => format!("%{other:02X}"),
        })
        .collect();
    format!("{name}={escaped}")
}

/// The path of a sign-in link the server printed.
fn page(link: &str) -> String {
    link[link.find("/login/").unwrap()..].to_owned()
}

/// A page's structure: one `h1`, and a label or aria-label for every control a person types in.
fn accessible(got: &Got) {
    let body = got.body();
    assert_eq!(body.matches("<h1").count(), 1, "one h1: {body}");
    let typed = body.matches("<input").count() - body.matches("type=\"hidden\"").count();
    let named = body.matches("<label").count() + body.matches("aria-label=\"").count();
    assert!(typed == 0 || named >= typed, "every input is labelled: {body}");
}

/// Signs `browser` in with the newest link the server printed, as the page does: a form post.
fn enter(server: &Server, browser: &mut Browser<'_>) -> Got {
    let link = server.links().pop().unwrap();
    let path = page(&link);
    let seen = server.http(("GET", &path), &[], "");
    assert_eq!(seen.status(), "200", "{}", seen.text);
    assert!(seen.sets_none(), "looking at a link signs nobody in");
    accessible(&seen);
    let entered = browser.send(("POST", &path), "");
    assert_eq!(entered.status(), "303", "{}", entered.text);
    assert!(entered.set_cookie().starts_with("ink-session="), "{}", entered.text);
    browser.keep(&entered);
    entered
}

/// Asks for a link for `address` through the sign-in page and waits for the mail to print.
fn ask(server: &Server, address: &str) {
    let before = server.links().len();
    let asked = server.browser().send(("POST", "/signin"), &field("address", address));
    assert_eq!(asked.status(), "303", "{}", asked.text);
    assert_eq!(asked.header("location"), Some("/signin/sent"));
    let started = std::time::Instant::now();
    while server.links().len() == before {
        assert!(started.elapsed() < Server::PATIENCE, "no mail: {}", server.said());
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[test]
fn the_owner_signs_in_pushes_a_folder_and_sees_the_deploy_and_its_build() {
    let server = Server::start("dash-owner", &["--site-ttl", "0"]);
    let mut owner = server.browser();

    let nobody = owner.send(("GET", "/account"), "");
    assert_eq!(nobody.status(), "401", "a page asks for a session: {}", nobody.text);
    assert!(nobody.body().contains("href=\"/signin\""), "and says where to sign in: {}", nobody.body());
    let form = owner.send(("GET", "/signin"), "");
    assert_eq!(form.status(), "200", "{}", form.text);
    accessible(&form);
    assert!(form.body().contains("passkey"), "the sign-in page offers the passkey script: {}", form.body());

    enter(&server, &mut owner);
    assert!(owner.send(("GET", "/v1/session"), "").status() == "200", "the page's session is the API's");
    let home = owner.send(("GET", "/"), "");
    assert_eq!(home.status(), "303", "{}", home.text);
    let account = owner.send(("GET", "/account"), "");
    assert_eq!(account.status(), "200", "{}", account.text);
    accessible(&account);

    let machine = {
        let mut machine = server.machine("laptop");
        machine.dir = server.root.join("site");
        std::fs::create_dir_all(&machine.dir).unwrap();
        machine
    };
    let login = machine.ink(&["login", "--data", server.data.to_str().unwrap(), "--label", "e2e"]);
    assert_eq!(login.code, 0, "{}{}", login.out, login.err);
    machine.write("index.md", "# One\n");
    let init = machine.ink(&["init", "--handle", "diary", "--json"]);
    assert_eq!(init.code, 0, "{}{}", init.out, init.err);
    let pushed = machine.ink(&["push", "--wait", "30", "--json"]);
    assert_eq!(pushed.code, 0, "{}{}{}", pushed.out, pushed.err, server.said());
    let (deploy, build) =
        (pushed.json()["deploy"].as_str().unwrap().to_owned(), pushed.json()["build"].as_str().unwrap().to_owned());

    let account = owner.send(("GET", "/account"), "");
    assert!(account.body().contains("diary"), "the site list has the site: {}", account.body());
    let site = owner.send(("GET", "/diary"), "");
    assert_eq!(site.status(), "200", "{}", site.text);
    accessible(&site);
    assert!(site.body().contains(&deploy[..8]), "the live deploy: {}", site.body());
    let deploys = owner.send(("GET", "/diary/deploys"), "");
    assert_eq!(deploys.status(), "200", "{}", deploys.text);
    accessible(&deploys);
    assert!(deploys.body().contains(&build[..8]), "the build is listed: {}", deploys.body());
    let log = owner.send(("GET", &format!("/diary/deploys/{build}")), "");
    assert_eq!(log.status(), "200", "{}", log.text);
    accessible(&log);
    for screen in ["/diary/understood", "/diary/settings"] {
        let got = owner.send(("GET", screen), "");
        assert_eq!(got.status(), "200", "{screen}: {}", got.text);
        accessible(&got);
    }

    // a state change from another origin is refused, and changes nothing
    let foreign = owner.send_with(("POST", "/diary/build"), &[("Origin", "http://evil.example")], "");
    assert_eq!(foreign.status(), "403", "{}", foreign.text);
    let rebuilt = owner.send(("POST", "/diary/build"), "");
    assert_eq!(rebuilt.status(), "409", "{}", rebuilt.text);
    assert!(rebuilt.body().contains("site.source.upload"), "an upload has nothing to build again: {}", rebuilt.body());

    // a second push, then a rollback to the first deploy by a form
    machine.write("index.md", "# Two\n");
    let second = machine.ink(&["push", "--wait", "30", "--json"]);
    assert_eq!(second.code, 0, "{}{}{}", second.out, second.err, server.said());
    assert_ne!(second.json()["deploy"].as_str().unwrap(), deploy);
    let back = owner.send(("POST", "/diary/rollback"), &field("deploy", &deploy));
    assert_eq!(back.status(), "303", "{}", back.text);
    let site = owner.send(("GET", "/diary"), "");
    assert!(site.body().contains(&deploy[..8]), "the first deploy is live again: {}", site.body());
    let export = owner.send(("GET", "/diary/export"), "");
    assert_eq!(export.status(), "200", "{}", export.text);

    // a site that is not the caller's, or does not exist, is the same page
    assert_eq!(owner.send(("GET", "/nothere"), "").status(), "404");

    let out = owner.send(("POST", "/logout"), "");
    assert_eq!(out.status(), "303", "{}", out.text);
    owner.keep(&out);
    assert_eq!(owner.send(("GET", "/account"), "").status(), "401", "signed out");
}

#[test]
fn a_person_ends_another_session_signs_out_and_mints_recovery_codes() {
    let server = Server::start("dash-ann", &["--signup", "open"]);
    let (mut here, mut there) = (server.browser(), server.browser());
    ask(&server, "ann@example.org");
    enter(&server, &mut here);
    ask(&server, "ann@example.org");
    enter(&server, &mut there);

    let account = here.send(("GET", "/account"), "");
    assert_eq!(account.status(), "200", "{}", account.text);
    accessible(&account);
    let body = account.body();
    assert_eq!(body.matches("/account/sessions/").count(), 1, "the other session has an end form, this one has none");

    // recovery codes: the page's script posts, the account shows how many are left
    let minted = here.send(("POST", "/v1/recovery/codes"), "");
    assert_eq!(minted.status(), "201", "{}", minted.text);
    let account = here.send(("GET", "/account"), "");
    assert!(account.body().contains("10"), "ten codes are left: {}", account.body());

    // end the other session: the page that lists it has a form per session
    let mine = here.send(("GET", "/v1/sessions"), "").json();
    let other = mine.as_array().unwrap().iter().find(|item| item["current"] == false).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let ended = here.send(("POST", &format!("/account/sessions/{other}")), "");
    assert_eq!(ended.status(), "303", "{}", ended.text);
    assert_eq!(there.send(("GET", "/account"), "").status(), "401", "the other session is over");
    assert_eq!(here.send(("GET", "/account"), "").status(), "200", "this one is not");

    let out = here.send(("POST", "/logout"), "");
    assert_eq!(out.status(), "303", "{}", out.text);
    here.keep(&out);
    assert_eq!(here.send(("GET", "/account"), "").status(), "401");
}

#[test]
fn the_origin_is_derived_from_the_listen_address_and_said() {
    let server = Server::derived("dash-derived", &[]);
    let said = server.said();
    assert!(
        said.contains(&format!("http://localhost:{} (from the listen address", server.port)),
        "the start says the origin it chose: {said}"
    );
    let seen = server.browser().send(("GET", "/signin"), "");
    assert_eq!(seen.status(), "200", "{}", seen.text);
}

#[test]
fn a_face_or_a_stroke_is_served_to_anyone_cached_and_with_no_cookie() {
    let server = Server::start("dash-assets", &[]);
    let browser = server.browser();
    for (path, kind) in [
        ("/gaegu-regular.woff2", "font/woff2"),
        ("/outline-1.svg", "image/svg+xml"),
        ("/outline-small.svg", "image/svg+xml"),
        ("/token.css", "text/css; charset=utf-8"),
    ] {
        let got = browser.send(("GET", path), "");
        assert_eq!(got.status(), "200", "{path}: {}", got.text);
        assert_eq!(got.header("content-type"), Some(kind), "{path}");
        assert_eq!(got.header("cache-control"), Some("public, max-age=86400"), "{path}");
        assert_eq!(got.header("set-cookie"), None, "{path}: a public file sets no cookie");
    }
    let page = browser.send(("GET", "/signin"), "");
    let csp = page.header("content-security-policy").unwrap();
    assert!(csp.contains("font-src 'self'"), "{csp}");
    assert_eq!(page.header("cache-control"), Some("no-store"), "a page is still never kept");
}

#[test]
fn every_form_route_refuses_a_post_from_another_origin() {
    let server = Server::start("dash-origin", &[]);
    let browser = server.browser();
    let token = "0".repeat(64);
    let routes = [
        "/signin".to_owned(),
        format!("/login/{token}"),
        "/logout".to_owned(),
        "/account/sessions/0000000000000000000000000000000".to_owned(),
        "/diary/build".to_owned(),
        "/diary/rollback".to_owned(),
        "/diary/resume".to_owned(),
        "/nothere".to_owned(),
    ];
    for route in &routes {
        for header in [("Origin", "http://evil.example"), ("Sec-Fetch-Site", "cross-site")] {
            let refused = browser.send_with(("POST", route), &[header], "");
            assert_eq!(refused.status(), "403", "{route} with {header:?}: {}", refused.text);
            assert!(refused.body().contains("auth.origin.refused"), "{route}: {}", refused.body());
        }
    }
}
