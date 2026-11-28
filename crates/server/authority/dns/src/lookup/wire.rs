//! The DNS message, as much of RFC 1035 as a TXT lookup needs: one question out, the TXT records
//! of the answer back. The letters of the question's name take the case of random bits (the 0x20
//! encoding of draft-vixie-dnsext-dns0x20), and an answer is read only if it echoes the name in
//! exactly that case and holds exactly one question, so a forged answer has to guess them too. Everything read from the network is checked against the message's own
//! length, and a compressed name may only point backwards, so a hostile answer ends in a typed
//! error and never in a loop or a read past the end.

use std::fmt;

/// `QR`: the message is an answer.
const ANSWER: u16 = 0x8000;
/// `TC`: the answer was cut to fit a datagram.
const CUT: u16 = 0x0200;
/// `RD`: the server may ask others for us.
const RECURSION: u16 = 0x0100;
/// The record type TXT.
const TXT: u16 = 16;
/// The class IN.
const INTERNET: u16 = 1;
/// The longest name, in bytes of its text form.
const NAME: usize = 253;
/// The longest label.
const LABEL: u8 = 63;
/// How many compression pointers one name may follow.
const JUMPS: usize = 16;

/// Why a message is not one this reads.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Malformed {
    /// The message ends inside a field.
    Short {
        /// The offset the field starts at.
        at: usize,
    },
    /// A label longer than 63 bytes, or one that uses the reserved length bits.
    Label {
        /// The offset of the length byte.
        at: usize,
    },
    /// A compressed name that points forward, or follows too many pointers.
    Pointer {
        /// The offset of the pointer.
        at: usize,
    },
    /// The host to ask about is no name: empty, a label too long, or too long in all.
    Host {
        /// The text.
        host: String,
    },
    /// The id is not the one the question had.
    Id {
        /// What was sent.
        sent: u16,
        /// What came.
        got: u16,
    },
    /// The message is a question, not an answer.
    Question,
    /// The answer is about another name or another kind of record than the one asked.
    Echo,
}

impl fmt::Display for Malformed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Short { at } => write!(out, "the message ends inside the field at byte {at}"),
            Self::Label { at } => write!(out, "the label at byte {at} is longer than 63 bytes or is not a label"),
            Self::Pointer { at } => write!(out, "the name pointer at byte {at} points forward or loops"),
            Self::Host { host } => write!(out, "`{host}` is not a name a question can carry"),
            Self::Id { sent, got } => write!(out, "the answer has the id {got}, the question had {sent}"),
            Self::Question => out.write_str("the message is a question, not an answer"),
            Self::Echo => out.write_str("the answer is about another name or kind of record than the question"),
        }
    }
}

impl std::error::Error for Malformed {}

/// A question for the TXT records of one host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Query {
    pub(crate) id: u16,
    pub(crate) host: String,
}

/// What an answer holds that a lookup uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Reply {
    /// The answer was cut: ask again over a stream.
    pub(crate) cut: bool,
    /// The server's response code: 0 for none, 3 for a name that does not exist.
    pub(crate) code: u8,
    /// The text of each TXT record, its strings joined.
    pub(crate) texts: Vec<String>,
}

impl Query {
    /// How many letters of a name the bits of a mask set: the first byte's lowest bit is the
    /// first letter's. The letters past it stay lower case.
    pub(crate) const LETTERS: usize = 112;

    /// The question for `host` with the case of each letter set by a bit of `mask`: a set bit is
    /// upper case.
    pub(crate) fn cased(id: u16, host: &str, mask: &[u8]) -> Self {
        let mut letter = 0_usize;
        let host = host
            .chars()
            .map(|c| {
                if !c.is_ascii_alphabetic() {
                    return c;
                }
                let upper =
                    letter < Self::LETTERS && mask.get(letter / 8).is_some_and(|byte| byte >> (letter % 8) & 1 == 1);
                letter += 1;
                if upper { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() }
            })
            .collect();
        Self { id, host }
    }

    /// The question as a message.
    pub(crate) fn encode(&self) -> Result<Vec<u8>, Malformed> {
        let bad = || Malformed::Host { host: self.host.clone() };
        let trimmed = self.host.trim_end_matches('.');
        if trimmed.is_empty() || trimmed.len() > NAME {
            return Err(bad());
        }
        let mut out = Vec::with_capacity(18 + trimmed.len());
        out.extend_from_slice(&self.id.to_be_bytes());
        out.extend_from_slice(&RECURSION.to_be_bytes());
        out.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0]);
        for label in trimmed.split('.') {
            let length = u8::try_from(label.len()).ok().filter(|length| (1..=LABEL).contains(length));
            out.push(length.ok_or_else(bad)?);
            out.extend_from_slice(label.as_bytes());
        }
        out.push(0);
        out.extend_from_slice(&TXT.to_be_bytes());
        out.extend_from_slice(&INTERNET.to_be_bytes());
        Ok(out)
    }

    /// An answer to this question holding `records`, each a list of character-strings, with
    /// `flags` set besides the answer bit. For the tests of the code that sends questions.
    #[cfg(test)]
    pub(crate) fn answered(&self, records: &[&[&str]], flags: u16) -> Vec<u8> {
        let mut message = self.encode().unwrap_or_default();
        message[2..4].copy_from_slice(&(ANSWER | flags).to_be_bytes());
        message[6..8].copy_from_slice(&u16::try_from(records.len()).unwrap_or(0).to_be_bytes());
        for strings in records {
            // the owner is a pointer to the question's name at byte 12
            message.extend_from_slice(&[0xc0, 12]);
            message.extend_from_slice(&TXT.to_be_bytes());
            message.extend_from_slice(&INTERNET.to_be_bytes());
            message.extend_from_slice(&[0, 0, 0, 60]);
            let data: Vec<u8> = strings
                .iter()
                .flat_map(|s| std::iter::once(u8::try_from(s.len()).unwrap_or(0)).chain(s.bytes()))
                .collect();
            message.extend_from_slice(&u16::try_from(data.len()).unwrap_or(0).to_be_bytes());
            message.extend_from_slice(&data);
        }
        message
    }

    /// What `bytes` answer to this question.
    pub(crate) fn read(&self, bytes: &[u8]) -> Result<Reply, Malformed> {
        let cursor = Cursor(bytes);
        let got = cursor.word(0)?;
        if got != self.id {
            return Err(Malformed::Id { sent: self.id, got });
        }
        let flags = cursor.word(2)?;
        if flags & ANSWER == 0 {
            return Err(Malformed::Question);
        }
        let (questions, answers) = (cursor.word(4)?, cursor.word(6)?);
        // an answer that holds no question, or two, is not an answer to this one
        if questions != 1 {
            return Err(Malformed::Echo);
        }
        let (name, next) = cursor.name(12)?;
        let (kind, class) = (cursor.word(next)?, cursor.word(next + 2)?);
        // the case of the name is part of the question: a forger who cannot see it cannot echo it
        if name != self.host.trim_end_matches('.') || kind != TXT || class != INTERNET {
            return Err(Malformed::Echo);
        }
        let mut at = next + 4;
        let mut texts = Vec::new();
        for _ in 0..answers {
            let (_, next) = cursor.name(at)?;
            let (kind, class, length) =
                (cursor.word(next)?, cursor.word(next + 2)?, usize::from(cursor.word(next + 8)?));
            let start = next + 10;
            let data = cursor.slice(start, length)?;
            if kind == TXT && class == INTERNET {
                texts.push(Cursor::strings(&data, start)?);
            }
            at = start + length;
        }
        Ok(Reply { cut: flags & CUT != 0, code: (flags & 0x000f) as u8, texts })
    }
}

/// A message being read: every access is checked against its length.
#[derive(Clone, Copy, Debug)]
struct Cursor<'a>(&'a [u8]);

impl Cursor<'_> {
    fn slice(self, at: usize, length: usize) -> Result<Vec<u8>, Malformed> {
        let end = at.checked_add(length).ok_or(Malformed::Short { at })?;
        self.0.get(at..end).map(<[u8]>::to_vec).ok_or(Malformed::Short { at })
    }

    fn word(self, at: usize) -> Result<u16, Malformed> {
        match self.0.get(at..at + 2) {
            Some(&[high, low]) => Ok(u16::from_be_bytes([high, low])),
            _ => Err(Malformed::Short { at }),
        }
    }

    /// The name at `at` as dotted text, and the offset after it in the message (a pointer ends
    /// the name there, whatever it points to).
    fn name(self, at: usize) -> Result<(String, usize), Malformed> {
        let (mut text, mut here, mut next, mut jumps) = (String::new(), at, None, 0);
        loop {
            let length = *self.0.get(here).ok_or(Malformed::Short { at: here })?;
            match length {
                0 => return Ok((text, next.unwrap_or(here + 1))),
                1..=LABEL => {
                    let label = self.slice(here + 1, usize::from(length))?;
                    if !text.is_empty() {
                        text.push('.');
                    }
                    text.push_str(&String::from_utf8_lossy(&label));
                    if text.len() > NAME {
                        return Err(Malformed::Label { at: here });
                    }
                    here += 1 + usize::from(length);
                }
                192..=255 => {
                    let low = *self.0.get(here + 1).ok_or(Malformed::Short { at: here })?;
                    let target = (usize::from(length & 0x3f) << 8) | usize::from(low);
                    jumps += 1;
                    if target >= here || jumps > JUMPS {
                        return Err(Malformed::Pointer { at: here });
                    }
                    next.get_or_insert(here + 2);
                    here = target;
                }
                _ => return Err(Malformed::Label { at: here }),
            }
        }
    }

    /// The character-strings of a TXT record's data, joined.
    fn strings(data: &[u8], start: usize) -> Result<String, Malformed> {
        let (mut joined, mut at) = (Vec::new(), 0);
        while let Some(&length) = data.get(at) {
            let end = at + 1 + usize::from(length);
            let part = data.get(at + 1..end).ok_or(Malformed::Short { at: start + at })?;
            joined.extend_from_slice(part);
            at = end;
        }
        Ok(String::from_utf8_lossy(&joined).into_owned())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn query() -> Query {
        Query { id: 0x1234, host: "_uses-ink.example.org".to_owned() }
    }

    #[test]
    fn a_question_is_one_name_of_labels_and_the_txt_type() {
        let bytes = query().encode().unwrap();
        assert_eq!(&bytes[..4], &[0x12, 0x34, 0x01, 0x00]);
        assert_eq!(&bytes[12..13], &[9]);
        assert_eq!(&bytes[bytes.len() - 4..], &[0, 16, 0, 1]);
        let long = Query { id: 1, host: format!("{}.org", "a".repeat(64)) };
        assert!(matches!(long.encode(), Err(Malformed::Host { .. })));
        assert!(matches!(Query { id: 1, host: String::new() }.encode(), Err(Malformed::Host { .. })));
        assert!(matches!(Query { id: 1, host: "a..b".to_owned() }.encode(), Err(Malformed::Host { .. })));
    }

    #[test]
    fn the_strings_of_a_record_are_joined_and_other_records_are_skipped() {
        let query = query();
        let message = query.answered(&[&["uses-ink-domain=", "abc"], &["v=spf1"]], 0);
        let reply = query.read(&message).unwrap();
        assert_eq!((reply.cut, reply.code), (false, 0));
        assert_eq!(reply.texts, ["uses-ink-domain=abc", "v=spf1"]);
        let cut = query.read(&query.answered(&[], CUT | 3)).unwrap();
        assert_eq!((cut.cut, cut.code, cut.texts.len()), (true, 3, 0));
    }

    #[test]
    fn what_is_not_the_answer_to_this_question_is_refused() {
        let query = query();
        let message = query.answered(&[&["x"]], 0);
        let other = Query { id: 0x4321, ..query.clone() };
        assert_eq!(other.read(&message), Err(Malformed::Id { sent: 0x4321, got: 0x1234 }));
        let elsewhere = Query { host: "_uses-ink.example.net".to_owned(), ..query.clone() };
        assert_eq!(elsewhere.read(&message), Err(Malformed::Echo));
        assert_eq!(query.read(&query.encode().unwrap()), Err(Malformed::Question));
    }

    #[test]
    fn the_case_of_the_name_is_random_and_an_answer_must_echo_it() {
        let plain = Query::cased(7, "_uses-ink.example.org", &[0; 14]);
        assert_eq!(plain.host, "_uses-ink.example.org");
        let loud = Query::cased(7, "_uses-ink.example.org", &[0b0000_0101, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(loud.host, "_UsEs-ink.eXAMPLE.ORg");
        let message = loud.answered(&[&["x"]], 0);
        assert!(loud.read(&message).is_ok());
        // the same answer with the name folded to lower case is another question's
        assert_eq!(plain.read(&message), Err(Malformed::Echo));
        assert_eq!(Query { id: 7, ..plain.clone() }.read(&plain.answered(&[&["x"]], 0)).map(|r| r.texts.len()), Ok(1));
        // letters past the mask stay lower case
        let long = Query::cased(7, &"a".repeat(130), &[0xff; 14]);
        assert_eq!(long.host.chars().filter(char::is_ascii_uppercase).count(), Query::LETTERS);
    }

    #[test]
    fn an_answer_with_no_question_or_two_is_refused() {
        let query = query();
        let mut none = query.answered(&[&["x"]], 0);
        none[4..6].copy_from_slice(&0_u16.to_be_bytes());
        assert_eq!(query.read(&none), Err(Malformed::Echo));
        let mut two = query.answered(&[&["x"]], 0);
        two[4..6].copy_from_slice(&2_u16.to_be_bytes());
        assert_eq!(query.read(&two), Err(Malformed::Echo));
    }

    #[test]
    fn a_hostile_message_ends_in_an_error() {
        let query = query();
        let message = query.answered(&[&["x"]], 0);
        for cut in 0..message.len() {
            assert!(query.read(&message[..cut]).is_err(), "a message cut at {cut} was read");
        }
        // an answer whose owner points at itself
        let mut looping = query.answered(&[&["x"]], 0);
        let owner = looping.len() - 14;
        let [high, low] = u16::try_from(owner).unwrap().to_be_bytes();
        looping[owner] = 0xc0 | high;
        looping[owner + 1] = low;
        assert!(matches!(query.read(&looping), Err(Malformed::Pointer { .. })));
        // a label with the reserved length bits
        let mut reserved = query.answered(&[&["x"]], 0);
        reserved[12] = 0x80;
        assert!(matches!(query.read(&reserved), Err(Malformed::Label { .. })));
    }
}
