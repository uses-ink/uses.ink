//! Credentials: stored sealed, opened only when a build needs one.

use db::{Constraint, Db, Statement};
use secret::{Binding, Plain, Sealed, Secret};

use crate::credential::{self, Credential};
use crate::id::{Credential as Kind, Id, Site as SiteKind};
use crate::{Control, Error, Ports, Refusal};

impl<P: Ports> Control<P> {
    /// What a sealed credential is bound to: its own row id, so a sealed value copied to another
    /// row does not open (decision 0031).
    fn binding(id: Id<Kind>) -> Binding {
        Binding::from(id.to_string().as_str())
    }

    /// Seals a credential and stores it. The secret is zeroed when `new` is dropped; what is
    /// stored is the sealed bytes, and what comes back says nothing of the secret.
    pub async fn add_credential(&self, new: credential::New) -> Result<Credential, Error<P>> {
        const INSERT: &str = "INSERT INTO credential (id, site_id, kind, sealed, created_at) VALUES (?, ?, ?, ?, ?)";
        self.active(new.site).await?;
        let sealed = self.ports.secret().seal(&new.secret, &Self::binding(new.id)).await?;
        let insert = Statement::prepare(INSERT)
            .bind(new.id.to_string())
            .bind(new.site.to_string())
            .bind(new.purpose.name())
            .bind(sealed.as_ref())
            .bind(new.at.as_millisecond());
        match self.ports.db().run(insert).await {
            Ok(_) => Ok(Credential { id: new.id, site: new.site, purpose: new.purpose, created: new.at }),
            Err(error) if error.constraint() == Some(Constraint::Unique) => Err(Refusal::Exists { id: new.id.to_string() }.into()),
            Err(error) => Err(error.into()),
        }
    }

    /// Opens a credential. This is the only way a secret leaves its row.
    pub async fn credential(&self, id: Id<Kind>) -> Result<Plain, Error<P>> {
        const SELECT: &str = "SELECT sealed FROM credential WHERE id = ?";
        let row = self.ports.db().first(Statement::prepare(SELECT).bind(id.to_string())).await?;
        let row = row.ok_or(Refusal::NoCredential { credential: id })?;
        let sealed = Sealed::from(row.get::<Vec<u8>>("sealed")?);
        Ok(self.ports.secret().open(&sealed, &Self::binding(id)).await?)
    }

    /// What is stored for a site, oldest first, without the secrets.
    pub async fn credentials(&self, site: Id<SiteKind>) -> Result<Vec<Credential>, Error<P>> {
        const SELECT: &str = "SELECT id, site_id, kind, created_at FROM credential WHERE site_id = ? ORDER BY created_at, id";
        let rows = self.ports.db().rows(Statement::prepare(SELECT).bind(site.to_string())).await?;
        Ok(rows.iter().map(Credential::try_from).collect::<Result<_, _>>()?)
    }
}

