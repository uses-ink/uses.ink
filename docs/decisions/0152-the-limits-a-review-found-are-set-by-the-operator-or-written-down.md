---
number: "0152"
title: "The limits a review found are set by the operator or written down"
status: accepted
date: 2026-11-28
area: security
supersedes: []
---

## 0152 The limits a review found are set by the operator or written down

Accepted (2026-11-28). A review of the control plane and the edge named four weak spots that no earlier decision
settles. Each is either given a limit the operator sets, or its blast radius is written here and in the decision it
belongs to, so nobody finds it later by accident.

**Abuse reports are capped over every source (0089, 0100).** `POST /v1/abuse` takes a report from anyone, and the
per-source rate (`INK_ABUSE_LIMIT`) bounds one source, not many. `Policy::reports` (`--limit-reports`,
`INK_LIMIT_REPORTS`, 10,000 by default) bounds the **open** reports: a report past it is refused whole with
`abuse.reports.full`, status 503, and nothing is stored. The count and the insert are one statement, so two reports at
once cannot both take the last place. A report the operator actioned by suspending a site stops counting.
*Left:* only a suspension moves a report out of `open`, and there is no verb to dismiss one, so a flood that fills
the cap with reports about no site holds it until the operator raises the limit. A `dismissed` state needs a new
column constraint, which the database cannot `ALTER`; it is the next step if a flood happens.

**The DNS proof reads one path and checks no signature (0093, 0101).** The lookup asks the nameservers over plain UDP,
so whoever can answer them in time proves a name. Three things now make that harder, and one is written down:

- The letters of the question's name take a random case, one bit each from the same draw as the id (the 0x20
  encoding), and an answer is read only if it echoes the name in that exact case. A resolver that folds the case of
  the question it echoes gets no answer from this lookup and times out; that is how a forger without sight of the
  question fails, and the cost for such a nameserver is the lookup failing, not a wrong proof.
- An answer must hold exactly one question. A reply with none used to skip the question check.
- `--resolver-confirm ADDR[,ADDR]` (`INK_RESOLVER_CONFIRM`) names a second set of nameservers. With it, the records are
  read from both and used only when they are the same; a different answer is `Failed::Disagree` with the two counts.
  A second path that is down is not agreement.
- **DNSSEC is not checked.** An answer is trusted as far as the path to the nameserver is. Two sets of nameservers on
  different networks is the defence the operator has today; validating the chain is a larger piece of work with a
  root-key story of its own, and is not scheduled.

**The enrolment gate is one switch for everyone (0068, section 8).** The gate that closes enrolment after wrong codes
or too many waiting keys is global. The per-source rate on `POST /v1/enrol` keeps one address from filling it alone, and
the test `one_source_asking_to_enrol_cannot_close_the_gate_for_the_others` holds that. What stays: a signed-in person
who sends five wrong codes to the approval endpoint closes it for everyone for a minute, and can do it again each
minute; and about four sources, each within its own rate, fill the 20 waiting keys. A per-source cap on waiting keys
needs the source stored on the key row, which the table has no column for and cannot gain. Sign-up is open, so the
first case needs only an account. This is accepted, as 0068 accepted a global gate, with the operator's remedy being
`--enrol-limit` and the gate's own numbers; the note is added to 0068.

**A preview push counts the account's blobs (0088, 0091).** `Control::stored` lists the blobs of every site of the
account to see what a push would take it to, once for a preview and up to three times for a deploy over the limit.
The cost is a listing as long as the account's blobs. What bounds it: the number of sites, deploys kept and files per
upload are limits (`--limit-sites`, `--limit-deploys`, the archive limits), and a push is a signed request under its
key's nonce bound. It is not cached because a cached sum would let a push over the budget in the window a stale
number stays, which is the refusal 0088 exists for. If listings get slow, the next step is a per-account sum row
written in the batch that records a deploy.
