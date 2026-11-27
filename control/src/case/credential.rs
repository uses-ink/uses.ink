//! Credentials: stored sealed, opened only when a build needs one.

use db::{Constraint, Db};
use sea_query::{Expr, ExprTrait, Order, Query};
use secret::{Binding, Plain, Sealed, Secret};

use crate::credential::{self, Credential};
use crate::id::{Credential as Kind, Id, Site as SiteKind};
use crate::{Control, Error, Ports, Refusal, table};

impl<P: Ports> Control<P> {
    /// What a sealed credential is bound to: its own row id, so a sealed value copied to another
    /// row does not open (decision 0031).
    fn binding(id: Id<Kind>) -> Binding {
        Binding::from(id.to_string().as_str())
    }

    /// Seals a credential and stores it. The secret is zeroed when `new` is dropped; what is
    /// stored is the sealed bytes, and what comes back says nothing of the secret.
    pub async fn add_credential(&self, new: credential::New) -> Result<Credential, Error<P>> {
        self.active(new.site).await?;
        let sealed = self.ports.secret().seal(&new.secret, &Self::binding(new.id)).await?;
        let insert = Query::insert()
            .into_table(table::Credential::Table)
            .columns([
                table::Credential::Id,
                table::Credential::SiteId,
                table::Credential::Kind,
                table::Credential::Sealed,
                table::Credential::CreatedAt,
            ])
            .values_panic([
                Expr::val(new.id.to_string()),
                Expr::val(new.site.to_string()),
                Expr::val(new.purpose.name()),
                Expr::val(sealed.as_ref()),
                Expr::val(new.at.as_millisecond()),
            ])
            .to_owned();
        match self.ports.db().run(insert.into()).await {
            Ok(_) => Ok(Credential { id: new.id, site: new.site, purpose: new.purpose, created: new.at }),
            Err(error) if error.constraint() == Some(Constraint::Unique) => {
                Err(Refusal::Exists { id: new.id.to_string() }.into())
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Opens a credential. This is the only way a secret leaves its row.
    pub async fn credential(&self, id: Id<Kind>) -> Result<Plain, Error<P>> {
        let select = Query::select()
            .column(table::Credential::Sealed)
            .from(table::Credential::Table)
            .and_where(Expr::col(table::Credential::Id).eq(id.to_string()))
            .to_owned();
        let row = self.ports.db().first(select.into()).await?;
        let row = row.ok_or(Refusal::NoCredential { credential: id })?;
        let sealed = Sealed::from(row.get::<Vec<u8>>(table::Credential::Sealed)?);
        Ok(self.ports.secret().open(&sealed, &Self::binding(id)).await?)
    }

    /// What is stored for a site, oldest first, without the secrets.
    pub async fn credentials(&self, site: Id<SiteKind>) -> Result<Vec<Credential>, Error<P>> {
        let select = Query::select()
            .columns([
                table::Credential::Id,
                table::Credential::SiteId,
                table::Credential::Kind,
                table::Credential::CreatedAt,
            ])
            .from(table::Credential::Table)
            .and_where(Expr::col(table::Credential::SiteId).eq(site.to_string()))
            .order_by(table::Credential::CreatedAt, Order::Asc)
            .order_by(table::Credential::Id, Order::Asc)
            .to_owned();
        let rows = self.ports.db().rows(select.into()).await?;
        Ok(rows.iter().map(Credential::try_from).collect::<Result<_, _>>()?)
    }
}
