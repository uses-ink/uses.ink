//! The hosts, as one thing to ask: a [`Fleet`] is every [`Member`], probed together.

use std::thread;

use crate::cli::console::Console;
use crate::cli::error::Error;
use crate::cli::name::HostName;
use crate::host::transport::Transport;
use crate::host::{Host, Member, Snapshot};

/// Every host.
#[derive(Debug)]
pub(crate) struct Fleet<T> {
    members: Vec<Member<T>>,
}

/// What probing every host found.
#[derive(Debug, Default)]
pub(crate) struct Probed {
    pub(crate) up: Vec<Snapshot>,
    /// Hosts with a wake command that did not answer: asleep, not broken.
    pub(crate) asleep: Vec<Host>,
    pub(crate) down: Vec<(HostName, Error)>,
}

impl<T: Transport + Sync> Fleet<T> {
    pub(crate) fn new(members: Vec<Member<T>>) -> Self {
        Self { members }
    }

    pub(crate) fn member(&self, name: &HostName) -> Option<&Member<T>> {
        self.members.iter().find(|member| &member.host.name == name)
    }

    #[cfg(test)]
    pub(crate) fn members(&self) -> &[Member<T>] {
        &self.members
    }

    /// Probes every host at once; a host that does not answer is reported down and left out, or
    /// asleep when it has a wake command.
    pub(crate) fn probe(&self, console: &Console) -> Probed {
        let results: Vec<_> = thread::scope(|scope| {
            let running: Vec<_> = self.members.iter().map(|member| scope.spawn(move || member.probe())).collect();
            running.into_iter().zip(&self.members).map(|(handle, member)| (member, handle.join())).collect()
        });
        let mut probed = Probed::default();
        for (member, joined) in results {
            match joined {
                Ok(Ok(snap)) => probed.up.push(snap),
                Ok(Err(Error::Call { .. })) if member.host.wake.is_some() => probed.asleep.push(member.host.clone()),
                Ok(Err(error)) => probed.down.push((member.host.name.clone(), error)),
                Err(_) => console.note(format!("probing `{}` panicked", member.host.name)),
            }
        }
        probed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::fake::{Fake, Reply};

    #[test]
    fn probing_the_fleet_reports_the_host_that_is_down_and_keeps_the_rest() {
        let up = Member::new(Fake::remote("up"), Fake::saying(vec![Reply::ok(Reply::PROBE)]));
        let down = Member::new(Fake::remote("down"), Fake::saying(vec![Reply { lines: vec![], code: 255 }]));
        let probed = Fleet::new(vec![up, down]).probe(&Console::default());
        assert_eq!(probed.up.len(), 1);
        assert_eq!(probed.down.first().map(|(name, _)| name.as_str()), Some("down"));
    }
}
