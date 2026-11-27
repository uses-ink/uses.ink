//! What the tree says: the wording of `ink`'s help, once.

use super::{Ending, Entry, Example, Group, Tree, Variable};
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
        about: "Usage error: a folder that is missing or cannot be listed, a path given wrong, an output folder `ink build` will not write into.",
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

const PUSH: &[Ending] = &[
    Ending { exit: Exit::Done, about: "Done: the deploy is live" },
    Ending { exit: Exit::Content, about: "The build was refused: the site does not build" },
    Ending { exit: Exit::Usage, about: "Wrong usage: the folder is linked to no site, or the folder cannot be read" },
    Ending { exit: Exit::Auth, about: "Not signed in, or the signature was refused" },
    Ending { exit: Exit::Network, about: "The API could not be reached, the build failed or the wait ran out; safe to run again" },
    Ending { exit: Exit::Conflict, about: "The deploy is recorded and not live, because publishing is paused after a rollback" },
];

const SOURCE_ENV: &[Variable] = &[];

const SERVE_ENV: &[Variable] = &[Variable {
    name: "INK_SMTP_PASSWORD",
    about: "The SMTP password for --mail smtp; never a flag, so it stays out of process lists. Every other sign-in setting is a flag with its own INK_ variable",
}];

const SITE_ENV: &[Variable] = &[
    Variable { name: "INK_API", about: "The address of the API, as --api" },
    Variable { name: "INK_SITE", about: "The site's id, as --site" },
    Variable {
        name: "INK_KEY",
        about: "The private key to sign requests with, for a CI job; never a flag, so it stays out of process lists. `ink login` keeps one in the config folder instead",
    },
    Variable { name: "INK_KEY_FILE", about: "A file that holds that key, in place of INK_KEY" },
];

impl Entry {
    /// A command that works, with nothing yet said about it but its name and what it does.
    pub(super) const fn built(name: &'static str, group: Group, about: &'static str) -> Entry {
        Entry {
            name,
            aliases: &[],
            group,
            hidden: false,
            about,
            detail: "",
            examples: &[],
            endings: READ,
            env: SOURCE_ENV,
            children: &[],
        }
    }
}

const AUTH: &[Ending] = &[
    Ending { exit: Exit::Done, about: "Done" },
    Ending {
        exit: Exit::Usage,
        about: "Wrong usage: no API named, a key that is none, a private key that would print to a terminal",
    },
    Ending { exit: Exit::Auth, about: "No key, or the API refused the signature, or nobody approved the key in time" },
    Ending { exit: Exit::Network, about: "The API could not be reached or failed; safe to retry" },
];

const KEY_CHILDREN: &[Entry] = &[
    Entry {
        examples: &[Example { line: "ink key list", about: "Kind, rights, end and last use of each key" }],
        endings: AUTH,
        env: SITE_ENV,
        ..Entry::built("list", Group::Publish, "The account's keys")
    },
    Entry {
        detail: "A person approves the new key, as with login. Its private key is written to --out, or printed once to a pipe, never to a terminal.",
        examples: &[Example {
            line: "ink key add --site 0123abcd --label ci --out ci.key",
            about: "A deploy key for one site, written to a file",
        }],
        endings: AUTH,
        env: SITE_ENV,
        ..Entry::built("add", Group::Publish, "Make a key for another machine, such as a CI job")
    },
    Entry {
        examples: &[Example { line: "ink key revoke 7QWM-C4TK-9R", about: "End that key at once" }],
        endings: AUTH,
        env: SITE_ENV,
        ..Entry::built("revoke", Group::Publish, "End a key")
    },
];

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
    Entry {
        detail: "Writes .uses/site, which holds the site's id and the API's address and no credential. A folder with no link refuses push, status, watch and export; the [account] table of the config file never stands in for it.",
        examples: &[
            Example { line: "ink init --handle garden", about: "Make a site that takes uploads, and link this folder to it" },
            Example { line: "ink init --site 0123abcd", about: "Link this folder to a site that exists" },
        ],
        endings: REMOTE,
        env: SITE_ENV,
        ..Entry::built("init", Group::Write, "Link this folder to a site, without publishing it")
    },
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
    Entry {
        detail: "Makes a key pair on this machine; only its public half and a fingerprint leave it. A person approves the fingerprint, or with --data the operator's file takes it. The private key stays in a file of yours (mode 0600) and is never printed.",
        examples: &[
            Example { line: "ink login --api http://api.localhost:8080", about: "Ask for approval, and wait for it" },
            Example {
                line: "ink login --data ./data --api http://api.localhost:8080",
                about: "Enrol as the operator of a local server, with no browser",
            },
        ],
        endings: AUTH,
        env: SITE_ENV,
        ..Entry::built("login", Group::Publish, "Sign in to uses.ink")
    },
    Entry {
        examples: &[Example { line: "ink logout", about: "End this machine's key and forget it" }],
        endings: AUTH,
        env: SITE_ENV,
        ..Entry::built("logout", Group::Publish, "Sign out: end this machine's key")
    },
    Entry {
        examples: &[Example { line: "ink whoami", about: "The account, the key and what it may do" }],
        endings: AUTH,
        env: SITE_ENV,
        ..Entry::built("whoami", Group::Publish, "Show who you are signed in as")
    },
    Entry {
        children: KEY_CHILDREN,
        examples: &[Example {
            line: "ink key list",
            about: "Every key of the account, and the day each was last used",
        }],
        endings: AUTH,
        env: SITE_ENV,
        ..Entry::built("key", Group::Publish, "List, make and end keys")
    },
    Entry {
        detail: "Sends only the files the site does not hold, so an interrupted push is resumed by running it again. Waits for the build and prints the deploy id; --no-wait prints the build id. A deploy that is recorded and not live exits 5.",
        examples: &[
            Example { line: "ink push", about: "Send the folder and wait for the deploy" },
            Example { line: "ink push --no-wait", about: "Queue the build and print its id" },
        ],
        endings: PUSH,
        env: SITE_ENV,
        ..Entry::built("push", Group::Publish, "Send the folder to its site and start a deploy")
    },
    Entry {
        examples: &[Example { line: "ink status --json", about: "The overview as one JSON document" }],
        endings: REMOTE,
        env: SITE_ENV,
        ..Entry::built("status", Group::Publish, "Show what is live and what is building")
    },
    Entry {
        detail: "Follows a build to its end: the one named, the one running, or the next the site starts. Prints the deploy id.",
        examples: &[Example { line: "ink watch", about: "Follow the build that is running, or wait for the next" }],
        endings: PUSH,
        env: SITE_ENV,
        ..Entry::built("watch", Group::Publish, "Follow a build of this site to its end")
    },
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
    Entry {
        detail: "Writes into a new or empty folder, through a staging folder beside it, so a failure leaves no partial folder. The live site's files are named by route, with `index` for the root.",
        examples: &[
            Example { line: "ink export out", about: "The live site's files, into out" },
            Example { line: "ink export --source src", about: "The files the last push sent" },
        ],
        endings: REMOTE,
        env: SITE_ENV,
        ..Entry::built("export", Group::Publish, "Download the site, or the files the last push sent, to a folder")
    },
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
