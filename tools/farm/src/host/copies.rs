//! The workspace directories a host holds for a repo: listing them, and removing one.

use crate::cli::error::Error;
use crate::cli::name::Segment;
use crate::host::Member;
use crate::host::transport::Transport;
use crate::host::transport::program::{Call, Program};
use crate::job::event::{Event, Gone};

/// A workspace directory on a host, and the kibibytes it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Found {
    pub(crate) name: Segment,
    pub(crate) kib: u64,
}

impl<T: Transport> Member<T> {
    /// Removes the directory of workspace `name` of `repo`; what became of it. The program
    /// refuses a path that resolves outside the host's directory, and says so.
    pub(crate) fn remove(&self, repo: &Segment, name: &Segment) -> Result<Gone, Error> {
        let args = [self.host.dir.as_str(), repo.as_str(), name.as_str()].map(str::to_owned).to_vec();
        let said = self.say(&Call::new(Program::Remove, args), "removing the copy")?;
        said.events
            .into_iter()
            .find_map(|event| if let Event::Gone(gone) = event { Some(gone) } else { None })
            .ok_or_else(|| Error::Silent { host: self.host.name.clone(), what: "what became of the copy" })
    }

    /// The workspace directories of `repo` on the host.
    pub(crate) fn copies(&self, repo: &Segment) -> Result<Vec<Found>, Error> {
        let args = [self.host.dir.as_str(), repo.as_str()].map(str::to_owned).to_vec();
        let said = self.say(&Call::new(Program::List, args), "listing the copies")?;
        let mut copies = Vec::new();
        for event in said.events {
            if let Event::Copy { name, kib } = event {
                let name = name.parse().map_err(|source| Error::Name { path: name.into(), source })?;
                copies.push(Found { name, kib });
            }
        }
        Ok(copies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::fake::{Fake, Reply};

    #[test]
    fn a_remove_asks_with_the_dir_the_repo_and_the_name_and_reads_what_became_of_it() {
        let member = Member::new(Fake::remote("petal"), Fake::saying(vec![Reply::ok(&["@@farm:removed 2048"])]));
        let gone = member.remove(&"uses.ink".parse().unwrap(), &"old".parse().unwrap()).unwrap();
        assert_eq!(gone, Gone::Removed { kib: 2048 });
        let calls = member.transport().calls.lock().unwrap();
        assert_eq!(calls.first().unwrap().program, Program::Remove);
        assert_eq!(calls.first().unwrap().args, ["farm", "uses.ink", "old"]);
    }

    #[test]
    fn a_host_that_does_not_say_what_became_of_the_copy_is_an_error() {
        let member = Member::new(Fake::remote("petal"), Fake::saying(vec![Reply::ok(&[])]));
        let error = member.remove(&"r".parse().unwrap(), &"old".parse().unwrap()).unwrap_err();
        assert!(matches!(error, Error::Silent { .. }), "{error}");
    }

    #[test]
    fn the_list_is_read_into_names_and_sizes() {
        let said = Reply::ok(&["@@farm:copy a 10", "@@farm:copy b.2 0"]);
        let member = Member::new(Fake::remote("petal"), Fake::saying(vec![said]));
        let copies = member.copies(&"r".parse().unwrap()).unwrap();
        let shown: Vec<(&str, u64)> = copies.iter().map(|copy| (copy.name.as_str(), copy.kib)).collect();
        assert_eq!(shown, [("a", 10), ("b.2", 0)]);
    }
}
