//! What the tree says: the wording of `ink`'s help, once.

use super::{Ending, Entry, Example, Group, Status, Tree, Variable};
use crate::exit::Exit;

/// Rows of the closed set of exit codes, as `ink help exit` prints them.
const CLOSED: &[Ending] = &[
    Ending { exit: Exit::Done, about: "Done. Warnings, if any, were printed." },
    Ending {
        exit: Exit::Content,
        about: "Content error: the build or check failed on diagnostics, or `build --check` found the two builds differ. Nothing changed live; `ink build` wrote nothing.",
    },
    Ending {
        exit: Exit::Usage,
        about: "Usage error: a folder that is missing or cannot be listed, a path given wrong, an output folder `ink build` will not write into, or a verb that is not built yet.",
    },
    Ending { exit: Exit::Auth, about: "Not signed in, or access to a source was lost." },
    Ending { exit: Exit::Network, about: "Network or service unavailable. Safe to retry." },
    Ending {
        exit: Exit::Conflict,
        about: "Conflict: the site is paused after a rollback, a mount conflict, an unpushed commit.",
    },
    Ending { exit: Exit::Strict, about: "`--strict` was given and there were warnings. Nothing was written." },
];

const DONE: Ending = Ending { exit: Exit::Done, about: "Done; warnings, if any, were printed" };
const ERRORS: Ending = Ending { exit: Exit::Content, about: "The site has errors" };
const WRONG: Ending =
    Ending { exit: Exit::Usage, about: "Wrong usage, or a folder that is missing; nothing was changed" };

const READ: &[Ending] = &[DONE, ERRORS, WRONG];

const CHECK: &[Ending] =
    &[DONE, ERRORS, WRONG, Ending { exit: Exit::Strict, about: "--strict was given and there were warnings" }];

const BUILD: &[Ending] = &[
    Ending { exit: Exit::Done, about: "Built; warnings, if any, were printed" },
    Ending {
        exit: Exit::Content,
        about: "The site has errors, or --check found the two builds differ; nothing was written",
    },
    Ending {
        exit: Exit::Usage,
        about: "Wrong usage, or an output folder ink will not write into; nothing was changed",
    },
    Ending { exit: Exit::Strict, about: "--strict was given and there were warnings; nothing was written" },
];

const SERVER: &[Ending] = &[
    Ending { exit: Exit::Done, about: "Stopped with Ctrl-C" },
    Ending { exit: Exit::Usage, about: "The folder is no deploy, or the address cannot be used" },
];

const REMOTE: &[Ending] = &[
    Ending { exit: Exit::Done, about: "Done" },
    Ending { exit: Exit::Usage, about: "Wrong usage: no site named, an id that is none, a deploy the site lacks" },
    Ending { exit: Exit::Auth, about: "Not signed in, or the token was refused" },
    Ending { exit: Exit::Network, about: "The API could not be reached or failed; safe to retry" },
    Ending { exit: Exit::Conflict, about: "The API refused the change as it stands: the deploy is already live, say" },
];

const PENDING: &[Ending] = &[Ending { exit: Exit::Usage, about: "Not built yet; nothing was done" }];

const SOURCE_ENV: &[Variable] = &[];

const SERVE_ENV: &[Variable] = &[Variable {
    name: "INK_SMTP_PASSWORD",
    about: "The SMTP password for --mail smtp; never a flag, so it stays out of process lists. Every other sign-in setting is a flag with its own INK_ variable",
}];

const SITE_ENV: &[Variable] = &[
    Variable { name: "INK_API", about: "The address of the API, as --api" },
    Variable { name: "INK_SITE", about: "The site's id, as --site" },
    Variable {
        name: "INK_TOKEN",
        about: "The owner token the API checks; never a flag, so it stays out of process lists",
    },
];

impl Entry {
    /// A command that has not been built: it is in help, marked, and refuses with a typed error.
    pub(super) const fn pending(name: &'static str, group: Group, about: &'static str) -> Entry {
        Entry {
            name,
            aliases: &[],
            group,
            hidden: false,
            about,
            detail: "",
            status: Status::Pending { milestone: "M3" },
            examples: &[],
            endings: PENDING,
            env: SOURCE_ENV,
            children: &[],
        }
    }

    /// A command that works, with nothing yet said about it but its name and what it does.
    pub(super) const fn built(name: &'static str, group: Group, about: &'static str) -> Entry {
        Entry {
            name,
            aliases: &[],
            group,
            hidden: false,
            about,
            detail: "",
            status: Status::Built,
            examples: &[],
            endings: READ,
            env: SOURCE_ENV,
            children: &[],
        }
    }
}

const DEPLOY_CHILDREN: &[Entry] = &[
    Entry {
        examples: &[Example { line: "ink deploy list --limit 5", about: "The five newest deploys" }],
        endings: REMOTE,
        env: SITE_ENV,
        ..Entry::built("list", Group::Publish, "History, newest first (also: ink deploys)")
    },
    Entry {
        examples: &[Example {
            line: "ink deploy show 5kq3v9",
            about: "What that deploy built, and the log of its build",
        }],
        endings: REMOTE,
        env: SITE_ENV,
        ..Entry::built("show", Group::Publish, "One deploy: its build and its log")
    },
    Entry {
        examples: &[Example { line: "ink deploy rollback 4mq8c1", about: "Serve the earlier deploy again" }],
        endings: REMOTE,
        env: SITE_ENV,
        ..Entry::built("rollback", Group::Publish, "Make an earlier deploy the live one, and pause new deploys")
    },
    Entry {
        examples: &[Example { line: "ink deploy resume", about: "Deploy new pushes again" }],
        endings: REMOTE,
        env: SITE_ENV,
        ..Entry::built("resume", Group::Publish, "Let the site deploy new pushes again after a rollback")
    },
];

const ENTRIES: &[Entry] = &[
    Entry { ..Entry::pending("init", Group::Write, "Link this folder to a site, without publishing it") },
    Entry {
        aliases: &["c"],
        detail: "The folder is read and interpreted exactly as a build would; nothing is written.",
        examples: &[
            Example { line: "ink check", about: "Read this folder and list its problems" },
            Example { line: "ink check --strict", about: "The same, but a warning is an exit 6" },
        ],
        endings: CHECK,
        ..Entry::built("check", Group::Write, "Read the folder and report what is wrong; writes nothing")
    },
    Entry {
        aliases: &["b"],
        detail: "The folder is read, interpreted and rendered in full before the output folder is touched. A build that fails, or warns under --strict, writes nothing: the previous build stays as it was.",
        examples: &[
            Example { line: "ink build", about: "Build this folder into .ink/out" },
            Example { line: "ink build --strict", about: "The same, but a warning stops the build" },
            Example { line: "ink -C garden build -o public", about: "Build another folder into ./public" },
        ],
        endings: BUILD,
        ..Entry::built("build", Group::Write, "Build the folder into an output folder and print its deploy id")
    },
    Entry {
        examples: &[Example { line: "ink preview --port 4100", about: "Preview on 127.0.0.1:4100" }],
        endings: SERVER,
        ..Entry::built("preview", Group::Write, "Build in memory and serve it on 127.0.0.1 while you write")
    },
    Entry {
        detail: "Unlike preview, this serves what `ink build` already wrote, through the edge crate the product uses: conditional and range requests, the host and path refusals, the request and response budgets, and the manifest's headers as they are in production.",
        examples: &[
            Example { line: "ink serve", about: "Serve .ink/out of this folder on 127.0.0.1:8080" },
            Example { line: "ink -C garden serve", about: "Serve .ink/out of another folder" },
            Example {
                line: "ink serve --data state --git-scheme file --restart-limit 3",
                about: "Self-host: serve, watch and build git sites from disk, restarting a crashed loop three times",
            },
            Example {
                line: "ink serve --data state --mail smtp --mail-from ink@example.org --smtp-host mail.example.org",
                about: "Turn on the API and send sign-in links by SMTP",
            },
        ],
        endings: SERVER,
        env: SERVE_ENV,
        ..Entry::built("serve", Group::Write, "Serve a built deploy the way production does")
    },
    Entry {
        examples: &[
            Example { line: "ink explain posts/a.md", about: "Why one file looks the way it does" },
            Example { line: "ink explain posts/a.md --why page.title", about: "Replay one decision, rung by rung" },
        ],
        ..Entry::built("explain", Group::Understand, "Show the decisions behind the site, or behind one file")
    },
    Entry {
        examples: &[
            Example { line: "ink pin posts/a.md page.title", about: "Write that file's title decision into ink.toml" },
            Example { line: "ink pin site.title --dry-run", about: "Print the pin and write nothing" },
        ],
        ..Entry::built("pin", Group::Understand, "Write one decision's value into ink.toml")
    },
    Entry {
        examples: &[
            Example { line: "ink select", about: "List every page, whether it is published, and why" },
            Example { line: "ink select notes/trip.md", about: "Publish one page of a vault" },
        ],
        ..Entry::built("select", Group::Understand, "List what is published and why, or choose what to publish")
    },
    Entry::pending("login", Group::Publish, "Sign in to uses.ink"),
    Entry::pending("logout", Group::Publish, "Sign out"),
    Entry::pending("whoami", Group::Publish, "Show who you are signed in as"),
    Entry::pending("push", Group::Publish, "Send the folder to its site and start a deploy"),
    Entry::pending("status", Group::Publish, "Show what is live and what is building"),
    Entry::pending("watch", Group::Publish, "Wait for the next deploy of this commit and report it"),
    Entry {
        children: DEPLOY_CHILDREN,
        examples: &[
            Example { line: "ink deploy list --limit 5", about: "The five newest deploys" },
            Example { line: "ink deploy rollback 4mq8c1", about: "Serve the earlier deploy again" },
        ],
        endings: REMOTE,
        env: SITE_ENV,
        ..Entry::built("deploy", Group::Publish, "List, read, roll back and resume deploys")
    },
    Entry {
        hidden: true,
        endings: REMOTE,
        env: SITE_ENV,
        ..Entry::built("deploys", Group::Publish, "History of deploys, newest first")
    },
    Entry::pending("export", Group::Publish, "Write the whole site, source and history, to a folder"),
    Entry {
        examples: &[
            Example { line: "ink completions bash", about: "Print the bash script; see `ink help completions`" },
            Example { line: "ink completions nushell", about: "Print the nushell script" },
        ],
        ..Entry::built("completions", Group::Tools, "Print a completion script for your shell")
    },
    Entry::built("man", Group::Tools, "Print the manual page"),
    Entry {
        examples: &[
            Example { line: "ink help exit", about: "The exit codes" },
            Example { line: "ink help page.title", about: "What a decision or diagnostic code means" },
        ],
        ..Entry::built("help", Group::Tools, "Show this message, or the help of one command")
    },
];

const ROOT_ENV: &[Variable] = &[
    Variable { name: "INK_DIR", about: "The site's folder, as -C" },
    Variable { name: "INK_COLOR", about: "auto, always or never, as --color" },
    Variable { name: "INK_NO_PROGRESS", about: "Log lines only, as --no-progress" },
    Variable { name: "INK_LOG", about: "The log filter, as --log; RUST_LOG is not read" },
    Variable { name: "INK_LOG_FILE", about: "Also write the log as JSON lines here, as --log-file" },
    Variable { name: "INK_LOG_FORMAT", about: "pretty, compact or json, on stderr" },
    Variable { name: "INK_LOG_SPANS", about: "none, open, close or full" },
    Variable { name: "INK_LOG_TIMINGS", about: "1 prints the span timing table at the end" },
    Variable { name: "NO_COLOR", about: "Set and not empty: no colour" },
    Variable { name: "CLICOLOR_FORCE", about: "Set and not 0: colour even to a pipe" },
    Variable { name: "INK_COMPONENTS", about: "The folder the build script reads components from" },
];

pub(super) static INK: Tree = Tree {
    about: "Turn a folder of writing into a site, and publish it",
    examples: &[
        Example { line: "ink check", about: "Read this folder and list its problems" },
        Example { line: "ink build -o public", about: "Build into ./public" },
        Example { line: "ink explain posts/a.md", about: "Why one file looks the way it does" },
        Example { line: "ink select notes/trip.md", about: "Publish one page of a vault" },
    ],
    env: ROOT_ENV,
    endings: CLOSED,
    entries: ENTRIES,
};
