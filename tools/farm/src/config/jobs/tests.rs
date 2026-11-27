//! The job file of this repo and of small examples.

use super::*;
use crate::cli::error::Error;

const FILE: &str = "[job.lint]\nrun = \"cargo xtask check lint\"\ngolden = false\n\n[job.test]\nrun = \"cargo  xtask check test\"\ngolden = true\nlimit = 900\n";

#[test]
fn jobs_come_in_file_order_with_their_need_and_limit() {
    let jobs = Jobs::parse(FILE, PathBuf::new()).unwrap();
    let all = jobs.pick(&[]).unwrap();
    assert_eq!(all.iter().map(|recipe| recipe.name.as_str()).collect::<Vec<_>>(), ["lint", "test"]);
    assert_eq!(all[1].words, ["cargo", "xtask", "check", "test"]);
    assert_eq!((all[0].need, all[1].need), (Need::Any, Need::Golden));
    assert_eq!((all[0].limit, all[1].limit), (None, Some(Seconds(900))));
}

#[test]
fn a_job_may_cap_the_load_of_the_host_it_runs_on() {
    let text = format!("{FILE}max_load = 6\n");
    let all = Jobs::parse(&text, PathBuf::new()).unwrap().pick(&[]).unwrap();
    assert_eq!((all[0].ceiling, all[1].ceiling), (None, Some(Load(600))));
    let zero = format!("{FILE}max_load = 0\n");
    assert!(Jobs::parse(&zero, PathBuf::new()).is_err(), "a ceiling of zero would never place the job");
}

#[test]
fn a_pick_keeps_the_order_asked_and_an_unknown_job_is_usage() {
    let jobs = Jobs::parse(FILE, PathBuf::new()).unwrap();
    let asked: Vec<JobName> = vec!["test".parse().unwrap()];
    assert_eq!(jobs.pick(&asked).unwrap().len(), 1);
    let nope = jobs.pick(&["nope".parse().unwrap()]).unwrap_err();
    assert_eq!(nope.exit(), crate::cli::exit::Exit::Usage);
    assert!(nope.to_string().contains("declares no job `nope`"), "{nope}");
}

#[test]
fn this_repos_file_parses_and_has_golden_and_plain_jobs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let all = Jobs::load(&root).unwrap().pick(&[]).unwrap();
    assert!(all.iter().any(|job| job.need == Need::Golden) && all.iter().any(|job| job.need == Need::Any));
    assert!(all.iter().all(|job| job.words.first().is_some_and(|word| word == "cargo")));
}

#[test]
fn the_workspace_table_declares_disposable_names() {
    let text = format!("{FILE}\n[workspace]\ndisposable = [\".ink\", \"node_modules\"]\n");
    let jobs = Jobs::parse(&text, PathBuf::new()).unwrap();
    assert_eq!(jobs.disposable().iter().map(Disposable::as_str).collect::<Vec<_>>(), [".ink", "node_modules"]);
    assert_eq!(Jobs::parse(FILE, PathBuf::new()).unwrap().disposable(), &[] as &[Disposable]);
    for bad in [
        "disposable = [\"a/b\"]",
        "disposable = [\"..\"]",
        "disposable = [\"\"]",
        "disposable = [\"a b\"]",
        "keep = []",
    ] {
        let text = format!("{FILE}\n[workspace]\n{bad}\n");
        assert!(Jobs::parse(&text, PathBuf::new()).is_err(), "{bad}");
    }
}

#[test]
fn this_repo_declares_its_ink_cache_disposable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    assert!(Jobs::load(&root).unwrap().disposable().iter().any(|name| name.as_str() == ".ink"));
}

#[test]
fn inputs_cache_and_the_settings_tables_are_read_with_their_defaults() {
    let text = format!(
        "{FILE}\n[job.docs]\nrun = \"x\"\ngolden = false\ninputs = [\"**\", \"!docs/**\"]\ncache = false\n\n[cache]\nmax_age_hours = 2\nmax_entries = 5\n\n[budget]\nfull_checks_per_day = 3\n\n[sync]\nexclude = [\".jj\", \".impeccable\"]\n"
    );
    let jobs = Jobs::parse(&text, PathBuf::new()).unwrap();
    let all = jobs.pick(&[]).unwrap();
    assert!(all[0].inputs.is_none() && all[0].cache, "the whole tree and cached by default");
    assert!(all[2].inputs.as_ref().is_some_and(|files| files.selects("a.rs") && !files.selects("docs/a.md")));
    assert!(!all[2].cache);
    let settings = jobs.settings();
    assert_eq!((settings.limits.hours.get(), settings.limits.entries.get()), (2, 5));
    assert_eq!(settings.budget.map(NonZeroU32::get), Some(3));
    assert_eq!(settings.exclude.iter().map(crate::cli::name::Word::as_str).collect::<Vec<_>>(), [".jj", ".impeccable"]);
    let plain = Jobs::parse(FILE, PathBuf::new()).unwrap();
    let defaults = plain.settings();
    assert_eq!((defaults.limits.hours.get(), defaults.limits.entries.get(), defaults.budget), (168, 256, None));
    assert_eq!(defaults.exclude.iter().map(crate::cli::name::Word::as_str).collect::<Vec<_>>(), [".jj", ".git"]);
    assert_ne!(defaults.config, settings.config, "a changed file is a new config digest");
}

#[test]
fn a_bad_glob_names_the_job_and_the_line_of_the_file() {
    let text = "[job.lint]\nrun = \"x\"\ngolden = false\n\n[job.test]\nrun = \"y\"\ngolden = true\ninputs = [\n  \"**\",\n  \"src/[ab]\",\n]\n";
    let fault = Jobs::parse(text, PathBuf::from("/r/.config/farm.toml")).unwrap_err();
    let shown = fault.to_string();
    assert!(shown.contains("[job.test]") && shown.contains("line 10"), "{shown}");
    let cause = std::error::Error::source(&fault).unwrap().to_string();
    assert!(cause.contains("`src/[ab]`") && cause.contains('['), "{cause}");
    let error = Error::Jobs { path: PathBuf::from("/r/.config/farm.toml"), fault };
    assert!(error.to_string().contains("/r/.config/farm.toml"), "{error}");
    let empty = "[job.a]\nrun = \"x\"\ngolden = false\ninputs = []\n";
    assert!(Jobs::parse(empty, PathBuf::new()).unwrap_err().to_string().contains("line 4"));
    for bad in ["max_age_hours = 0", "max_entries = 0", "wat = 1"] {
        assert!(Jobs::parse(&format!("{FILE}\n[cache]\n{bad}\n"), PathBuf::new()).is_err(), "{bad}");
    }
    assert!(Jobs::parse(&format!("{FILE}\n[sync]\nexclude = [\"a/b\"]\n"), PathBuf::new()).is_err());
}

#[test]
fn this_repos_inputs_cover_every_crate_of_the_jobs_that_read_the_whole_workspace() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let all = Jobs::load(&root).unwrap().pick(&[]).unwrap();
    let manifest: toml::Table = toml::from_str(&std::fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
    let members: Vec<String> = manifest["workspace"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member.as_str().unwrap().to_owned())
        .collect();
    assert!(members.len() > 10);
    for name in ["lint", "test", "docs", "sites"] {
        let job = all.iter().find(|job| job.name.as_str() == name).unwrap();
        for member in &members {
            let file = format!("{member}/src/lib.rs");
            let selected = job.inputs.as_ref().is_none_or(|files| files.selects(&file));
            assert!(selected, "{name} does not read {file}: a new crate must not be served a stale pass");
        }
    }
    let test = all.iter().find(|job| job.name.as_str() == "test").unwrap();
    assert!(
        test.inputs.as_ref().is_some_and(|files| files.selects("docs/workflow.md")),
        "the farm's own test reads it"
    );
}

#[test]
fn a_bad_file_says_what_is_wrong() {
    for bad in [
        "",
        "[job.a]\nrun = \"\"\ngolden = true\n",
        "[job.a]\nrun = \"x\"\n",
        "[job.A]\nrun = \"x\"\ngolden = true\n",
        "[job.a]\nrun = \"x\"\ngolden = true\nlimit = 0\n",
        "[job.a]\nrun = \"x\"\ngolden = true\nextra = 1\n",
        "jobs = 1\n",
    ] {
        assert!(Jobs::parse(bad, PathBuf::new()).is_err(), "{bad:?}");
    }
}

#[test]
fn the_affected_table_and_a_narrowable_job_are_read_and_bad_globs_are_named() {
    let text = format!("{FILE}affected = true\n\n[affected]\nbase = \"main\"\nignore = [\"docs/**\", \"*.md\"]\n");
    let jobs = Jobs::parse(&text, PathBuf::new()).unwrap();
    let scope = &jobs.settings().scope;
    assert_eq!(scope.base.as_ref().map(crate::affected::Base::as_str), Some("main"));
    assert!(scope.ignore.selects("docs/a.png") && scope.ignore.selects("PRODUCT.md") && !scope.ignore.selects("a.rs"));
    let all = jobs.pick(&[]).unwrap();
    assert_eq!((all[0].narrow, all[1].narrow), (false, true));
    let bad = Jobs::parse(&format!("{FILE}\n[affected]\nignore = [\"a/**/b**\"]\n"), PathBuf::new()).unwrap_err();
    assert!(matches!(bad, Fault::Ignore { .. }), "{bad}");
    let bad = Jobs::parse(&format!("{FILE}\n[affected]\nbase = \"-x\"\n"), PathBuf::new()).unwrap_err();
    assert!(matches!(bad, Fault::Toml(_)), "{bad}");
}
