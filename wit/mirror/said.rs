// Places, subjects, problems and notices. Part of `wit/mirror`.

use model::{Diagnostic, Notice, Span, Subject, coded};

use super::refusal::{Malformed, Unmirrored};
use super::wit::model::diagnostic as said;
use super::written::Written;

impl From<Span> for said::Span {
    fn from(span: Span) -> Self {
        Self { start: span.start() as u64, end: span.end() as u64 }
    }
}

impl TryFrom<said::Span> for Span {
    type Error = Malformed;

    fn try_from(span: said::Span) -> Result<Self, Malformed> {
        let refused = Malformed::Span { start: span.start, end: span.end };
        let (Ok(start), Ok(end)) = (usize::try_from(span.start), usize::try_from(span.end)) else { return Err(refused) };
        Self::new(start, end).map_err(|_| refused)
    }
}

impl TryFrom<&Subject> for said::Subject {
    type Error = Unmirrored;

    fn try_from(subject: &Subject) -> Result<Self, Unmirrored> {
        match subject {
            Subject::Site => Ok(Self::Site),
            Subject::File(path) => Ok(Self::File(path.to_string())),
            _ => Err(Unmirrored::Subject),
        }
    }
}

impl TryFrom<said::Subject> for Subject {
    type Error = Malformed;

    fn try_from(subject: said::Subject) -> Result<Self, Malformed> {
        match subject {
            said::Subject::Site => Ok(Self::Site),
            said::Subject::File(path) => Written::read(path).map(Self::File),
        }
    }
}

// Problems and notices.

impl TryFrom<coded::Field> for said::Field {
    type Error = Unmirrored;

    fn try_from(field: coded::Field) -> Result<Self, Unmirrored> {
        Ok(match field {
            coded::Field::Path(path) => Self::Path(path.into()),
            coded::Field::Paths(paths) => Self::Paths(paths.into_iter().map(String::from).collect()),
            coded::Field::Route(route) => Self::Route(route.into()),
            coded::Field::Text(text) => Self::Text(text),
            coded::Field::Count(count) => Self::Count(count),
            coded::Field::Bytes(bytes) => Self::Bytes(bytes.count()),
            coded::Field::Span(span) => Self::Span(span.into()),
            coded::Field::Decision(code) => Self::Decision(code.name().to_owned()),
            _ => return Err(Unmirrored::Field),
        })
    }
}

impl TryFrom<said::Field> for coded::Field {
    type Error = Malformed;

    fn try_from(field: said::Field) -> Result<Self, Malformed> {
        Ok(match field {
            said::Field::Path(path) => Self::Path(Written::read(path)?),
            said::Field::Paths(paths) => Self::Paths(paths.into_iter().map(Written::read).collect::<Result<_, _>>()?),
            said::Field::Route(route) => Self::Route(Written::read(route)?),
            said::Field::Text(text) => Self::Text(text),
            said::Field::Count(count) => Self::Count(count),
            said::Field::Bytes(bytes) => Self::Bytes(model::Bytes(bytes)),
            said::Field::Span(span) => Self::Span(span.try_into()?),
            said::Field::Decision(code) => Self::Decision(Written::read(code)?),
        })
    }
}

impl TryFrom<coded::Coded> for said::Coded {
    type Error = Unmirrored;

    fn try_from(coded: coded::Coded) -> Result<Self, Unmirrored> {
        let fields = coded.fields.into_iter().map(said::Field::try_from).collect::<Result<_, _>>()?;
        Ok(Self { code: coded.code, fields })
    }
}

impl TryFrom<said::Coded> for coded::Coded {
    type Error = Malformed;

    fn try_from(coded: said::Coded) -> Result<Self, Malformed> {
        let fields = coded.fields.into_iter().map(coded::Field::try_from).collect::<Result<_, _>>()?;
        Ok(Self::new(coded.code, fields))
    }
}

impl TryFrom<&Diagnostic> for said::Diagnostic {
    type Error = Unmirrored;

    fn try_from(diagnostic: &Diagnostic) -> Result<Self, Unmirrored> {
        let chain = diagnostic
            .chain()
            .map(|link| {
                Ok(said::Said {
                    problem: coded::Coded::from(&link.problem).try_into()?,
                    subject: link.subject.as_ref().map(said::Subject::try_from).transpose()?,
                    span: link.span.map(said::Span::from),
                })
            })
            .collect::<Result<_, Unmirrored>>()?;
        Ok(Self { chain })
    }
}

impl TryFrom<said::Diagnostic> for Diagnostic {
    type Error = Malformed;

    /// The chain from its last cause back to the problem, so no input recurses.
    fn try_from(diagnostic: said::Diagnostic) -> Result<Self, Malformed> {
        let mut built: Option<Self> = None;
        for link in diagnostic.chain.into_iter().rev() {
            let problem = model::Problem::try_from(coded::Coded::try_from(link.problem)?)
                .map_err(|source| Malformed::Coded { source })?;
            let mut made = Self::new(problem);
            if let Some(subject) = link.subject {
                made = made.on(subject.try_into()?);
            }
            if let Some(span) = link.span {
                made = made.at(span.try_into()?);
            }
            if let Some(cause) = built.take() {
                made = made.caused_by(cause);
            }
            built = Some(made);
        }
        built.ok_or(Malformed::Chain)
    }
}

impl TryFrom<&Notice> for said::Notice {
    type Error = Unmirrored;

    fn try_from(notice: &Notice) -> Result<Self, Unmirrored> {
        Ok(Self {
            announcement: coded::Coded::from(&notice.announcement).try_into()?,
            subject: notice.subject.as_ref().map(said::Subject::try_from).transpose()?,
            span: notice.span.map(said::Span::from),
        })
    }
}

impl TryFrom<said::Notice> for Notice {
    type Error = Malformed;

    fn try_from(notice: said::Notice) -> Result<Self, Malformed> {
        let announcement = model::Announcement::try_from(coded::Coded::try_from(notice.announcement)?)
            .map_err(|source| Malformed::Coded { source })?;
        let mut made = Self::new(announcement);
        if let Some(subject) = notice.subject {
            made = made.on(subject.try_into()?);
        }
        if let Some(span) = notice.span {
            made = made.at(span.try_into()?);
        }
        Ok(made)
    }
}
