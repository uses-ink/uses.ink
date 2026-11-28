//! The flags after the target: `--width 1440,390` and `--from <folder>`.

use std::path::PathBuf;

use super::fault::Fault;

/// What a run was asked besides its target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Flags {
    /// The viewport widths, in pixels, one picture of every page each.
    pub(crate) widths: Vec<u32>,
    /// The folder of the mock, for the target `mock`; relative to the root when it is not absolute.
    pub(crate) from: Option<PathBuf>,
}

impl Flags {
    /// The widths of a run that gives none: a desktop and a phone.
    const WIDTHS: [u32; 2] = [1440, 390];

    /// Reads the flags: each is `--name value` or `--name=value`.
    pub(crate) fn parse(args: &[String]) -> Result<Self, Fault> {
        let mut flags = Self { widths: Self::WIDTHS.to_vec(), from: None };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let (name, inline) = match arg.split_once('=') {
                Some((name, value)) => (name, Some(value.to_owned())),
                None => (arg.as_str(), None),
            };
            let value = inline.or_else(|| args.next().cloned());
            match (name, value) {
                ("--width", Some(given)) => flags.widths = Self::widths(&given)?,
                ("--from", Some(given)) => flags.from = Some(PathBuf::from(given)),
                _ => return Err(Fault::Flag { given: arg.clone() }),
            }
        }
        Ok(flags)
    }

    /// A list of widths, `1440,390`.
    fn widths(given: &str) -> Result<Vec<u32>, Fault> {
        let widths = given
            .split(',')
            .map(|width| width.trim().parse::<u32>().map_err(|source| Fault::Width { given: width.to_owned(), source }))
            .collect::<Result<Vec<_>, _>>()?;
        if widths.is_empty() || widths.len() > 8 || widths.iter().any(|width| !(200..=4000).contains(width)) {
            return Err(Fault::Range { given: given.to_owned() });
        }
        Ok(widths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn defaults_to_desktop_and_phone() {
        assert_eq!(Flags::parse(&[]).expect("no flags").widths, vec![1440, 390]);
    }

    #[test]
    fn reads_both_spellings() {
        let flags = Flags::parse(&args(&["--width", "1024,320", "--from=mock"])).expect("flags");
        assert_eq!(flags.widths, vec![1024, 320]);
        assert_eq!(flags.from, Some(PathBuf::from("mock")));
    }

    #[test]
    fn refuses_what_it_does_not_know() {
        assert!(matches!(Flags::parse(&args(&["--wide"])), Err(Fault::Flag { .. })));
        assert!(matches!(Flags::parse(&args(&["--width"])), Err(Fault::Flag { .. })));
        assert!(matches!(Flags::parse(&args(&["--width", "wide"])), Err(Fault::Width { .. })));
        assert!(matches!(Flags::parse(&args(&["--width", "50"])), Err(Fault::Range { .. })));
    }
}
