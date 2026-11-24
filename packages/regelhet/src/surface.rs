//! Which side of the process an instance of the server serves: the citizens,
//! the employees of the back office, or both ([`Surface`]).
//!
//! Citizens and employees each get an environment of their own: an address,
//! a login and a deployment of their own, from the same image and on the
//! same database. `REGELHET_SURFACE` ([`crate::config`]) says which one an
//! instance is. Everything that differs between them is decided here, so
//! no other module asks for the surface by name:
//!
//! - which groups of routes exist ([`Surface::serves_citizens`],
//!   [`Surface::serves_employees`]). A route of the other side is not
//!   registered at all, so it answers 404 as any unknown path does, not 403;
//! - which roles a session keeps ([`Surface::admitted_roles`]): an instance
//!   for citizens never keeps an employee's role in a session, whatever the
//!   token carried, and one for employees never the citizen's. The session
//!   store filters them when it writes a session, and the principal of a
//!   request once more when it reads one;
//! - the name of the session cookie, and the mark a session carries of the
//!   surface it was made on ([`crate::login::SurfaceSessions`]), so that a
//!   session of one side is no session on the other although both live in
//!   the table `regelhet.sessions`;
//! - which accounts the test login offers: those of the sides it serves;
//! - which build of the screens the instance serves ([`Surface::name`], a
//!   directory under `REGELHET_FRONTEND`).
//!
//! [`Surface::All`] serves both sides, as one server did before the split.
//! It is for trying the process out on one machine, so stable mode refuses
//! it ([`crate::config`]).

use std::collections::BTreeSet;

use crate::access::CITIZEN;

/// The side of the process an instance serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// Citizens: the case, the forms, their own dossier and submissions,
    /// and FSC voor Burgers when it is configured.
    Citizen,
    /// Employees: the chronicles, the work queue, decisions and, in test
    /// mode, the clock.
    Backoffice,
    /// Both, on one address; test mode only.
    All,
}

impl Surface {
    /// Every surface.
    pub const ALL: [Self; 3] = [Self::Citizen, Self::Backoffice, Self::All];

    /// Its name, as `REGELHET_SURFACE` takes it and as the directory of
    /// its screens is called.
    pub fn name(self) -> &'static str {
        match self {
            Self::Citizen => "burger",
            Self::Backoffice => "backoffice",
            Self::All => "all",
        }
    }

    /// Whether the citizen's routes exist on this surface.
    pub fn serves_citizens(self) -> bool {
        matches!(self, Self::Citizen | Self::All)
    }

    /// Whether the routes of the back office exist on this surface.
    pub fn serves_employees(self) -> bool {
        matches!(self, Self::Backoffice | Self::All)
    }

    /// The name of the session cookie: one per surface, so a browser that
    /// visits both environments never sends the one's cookie to the other.
    pub fn session_cookie(self) -> &'static str {
        match self {
            Self::Citizen => "regelhet_burger_session",
            Self::Backoffice => "regelhet_backoffice_session",
            Self::All => "regelhet_session",
        }
    }

    /// Of the realm roles `roles`, those a session keeps on this surface.
    /// An instance for citizens keeps only the citizen's role; one for
    /// employees keeps every role but the citizen's. Another role of the
    /// realm, such as the access role, is no right of this server and stays
    /// where it does no harm.
    pub fn admitted_roles(self, roles: BTreeSet<String>) -> BTreeSet<String> {
        match self {
            Self::All => roles,
            Self::Citizen => roles.into_iter().filter(|r| r == CITIZEN).collect(),
            Self::Backoffice => roles.into_iter().filter(|r| r != CITIZEN).collect(),
        }
    }
}

impl std::fmt::Display for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::str::FromStr for Surface {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        Self::ALL
            .into_iter()
            .find(|surface| surface.name() == s)
            .ok_or_else(|| s.to_string())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn roles(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|r| r.to_string()).collect()
    }

    #[test]
    fn a_surface_reads_back_from_its_name() {
        for surface in Surface::ALL {
            assert_eq!(surface.name().parse::<Surface>().unwrap(), surface);
        }
        assert_eq!("medewerker".parse::<Surface>(), Err("medewerker".into()));
    }

    #[test]
    fn the_citizens_surface_keeps_only_the_citizens_role() {
        let token = roles(&[
            CITIZEN,
            "regelhet-toegang",
            "regelhet-admin",
            "regelhet-beslisser",
        ]);
        assert_eq!(
            Surface::Citizen.admitted_roles(token.clone()),
            roles(&[CITIZEN])
        );
        assert_eq!(
            Surface::Backoffice.admitted_roles(token.clone()),
            roles(&["regelhet-toegang", "regelhet-admin", "regelhet-beslisser",])
        );
        assert_eq!(Surface::All.admitted_roles(token.clone()), token);
    }

    #[test]
    fn every_surface_has_a_cookie_of_its_own() {
        let names: BTreeSet<&str> = Surface::ALL.map(Surface::session_cookie).into();
        assert_eq!(names.len(), Surface::ALL.len());
    }
}
