use std::collections::BTreeMap;

use model::git::Grant;

/// The credentials the builder holds, by the name a request gives them. A secret redacts itself
/// in `Debug` and has no `Display`; a plugin only ever names one.
#[derive(Clone, Debug, Default)]
pub struct Grants(BTreeMap<Grant, Secret>);

/// A user name and password, or a token as the password.
#[derive(Clone)]
pub(super) struct Secret {
    user: String,
    password: String,
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Secret(..)")
    }
}

impl Grants {
    /// Holds `password` for `user` under `grant`.
    pub fn hold(&mut self, grant: Grant, user: impl Into<String>, password: impl Into<String>) {
        self.0.insert(grant, Secret { user: user.into(), password: password.into() });
    }

    pub(super) fn secret(&self, grant: &Grant) -> Option<&Secret> {
        self.0.get(grant)
    }
}

impl Secret {
    /// gix's credential callback answering every request with this secret and storing nothing.
    pub(super) fn answer(
        &self,
    ) -> impl FnMut(gix::credentials::helper::Action) -> gix::Result<Option<gix::credentials::protocol::Outcome>> + use<>
    {
        let this = self.clone();
        move |action| {
            Ok(match action {
                gix::credentials::helper::Action::Get(context) => Some(gix::credentials::protocol::Outcome {
                    credentials: gix::sec::identity::Credentials::UsernamePassword {
                        username: this.user.clone(),
                        password: this.password.clone(),
                        oauth_refresh_token: None,
                    },
                    next: context.into(),
                }),
                _ => None,
            })
        }
    }
}
