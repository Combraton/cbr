//! Grant records and the authorization decision (CORE section 15).
//!
//! A grant is a provider-owned subject of kind `core.grant`, not a bearer
//! token: nothing a caller sends carries its own authority, and the record is
//! read from this provider's own store every time. That is what makes
//! revocation immediate rather than eventual.
//!
//! The decision this module returns is deliberately a **reason**, not a
//! boolean. CORE section 15.5 fixes the order reasons are reported in, and the
//! order is load-bearing: `grant_not_found` is decided before any other
//! property of the grant, so another principal's revoked grant is
//! indistinguishable from one that never existed. A `bool` cannot express
//! that, and an implementation that returns one will leak the difference.

use cbr_encoding::Value;

use crate::errors::ProtocolError;
use crate::store::SubjectKey;

/// The subject kind a grant is stored under.
pub const KIND: &str = "core.grant";

/// The one authority scope `core-test/1` defines (CORE section 13). A binding
/// naming anything else is `invalid_envelope`, because a provider cannot
/// evaluate a scope it does not track and must not pretend the binding holds.
pub const KNOWN_SCOPES: [&str; 1] = ["core-test"];

/// A resource a grant covers: a subject kind, optionally narrowed by exactly
/// one of an exact id or an id prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub kind: String,
    pub id: Option<String>,
    pub id_prefix: Option<String>,
}

impl Resource {
    fn covers(&self, key: &SubjectKey) -> bool {
        if self.kind != key.kind {
            return false;
        }
        match (&self.id, &self.id_prefix) {
            (Some(exact), _) => *exact == key.id,
            (None, Some(prefix)) => key.id.starts_with(prefix.as_str()),
            (None, None) => true,
        }
    }

    /// Whether `self` is at least as wide as `narrower`, for the delegation
    /// rule that every child resource is covered by a parent resource.
    fn covers_resource(&self, narrower: &Resource) -> bool {
        if self.kind != narrower.kind {
            return false;
        }
        match (&self.id, &self.id_prefix) {
            // An unnarrowed parent covers any narrowing of its kind.
            (None, None) => true,
            // An exact parent covers only the identical exact child.
            (Some(exact), _) => narrower.id.as_deref() == Some(exact.as_str()),
            (None, Some(prefix)) => match (&narrower.id, &narrower.id_prefix) {
                (Some(id), _) => id.starts_with(prefix.as_str()),
                (None, Some(child)) => child.starts_with(prefix.as_str()),
                // An unnarrowed child is wider than a prefixed parent.
                (None, None) => false,
            },
        }
    }

    fn to_value(&self) -> Value {
        let mut members = vec![("kind".into(), Value::String(self.kind.clone()))];
        if let Some(id) = &self.id {
            members.push(("id".into(), Value::String(id.clone())));
        }
        if let Some(prefix) = &self.id_prefix {
            members.push(("id_prefix".into(), Value::String(prefix.clone())));
        }
        Value::Object(members)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delegation {
    pub allowed: bool,
    pub max_depth: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub scope: String,
    pub epoch: i64,
}

#[derive(Debug, Clone)]
pub struct Grant {
    pub id: String,
    pub issuer: String,
    pub holder: String,
    pub audience: String,
    pub rights: Vec<String>,
    pub resources: Vec<Resource>,
    pub expires_at: Option<String>,
    pub authority_binding: Option<Binding>,
    pub delegation: Delegation,
    pub parent: Option<String>,
    pub revoked: bool,
}

/// Why an operation was refused, in the order CORE section 15.5 fixes.
///
/// The discriminant order is the report order, so a caller that collects
/// several and takes the smallest reports the right one without restating the
/// rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Denial {
    GrantNotFound,
    Revoked,
    Expired,
    AuthorityEpochStale,
    RightMissing,
    OutOfScope,
    DelegationExceeded,
    NotAuthority,
    GrantRequired,
}

impl Denial {
    pub fn reason(self) -> &'static str {
        match self {
            Denial::GrantNotFound => "grant_not_found",
            Denial::Revoked => "revoked",
            Denial::Expired => "expired",
            Denial::AuthorityEpochStale => "authority_epoch_stale",
            Denial::RightMissing => "right_missing",
            Denial::OutOfScope => "out_of_scope",
            Denial::DelegationExceeded => "delegation_exceeded",
            Denial::NotAuthority => "not_authority",
            Denial::GrantRequired => "grant_required",
        }
    }
}

impl From<Denial> for ProtocolError {
    fn from(denial: Denial) -> Self {
        ProtocolError::permission_denied(denial.reason())
    }
}

/// One right an operation needs, and the subject it is needed over.
pub type Need = (&'static str, Option<SubjectKey>);

/// Everything about the provider that a grant decision depends on, gathered
/// once so the decision itself is a pure function of it.
pub struct Context {
    /// The provider clock, read once per decision so one operation sees one
    /// instant even while a controlled clock file is being replaced.
    pub now: String,
    /// The current epoch of every authority scope the provider tracks.
    pub epochs: Vec<(String, i64)>,
}

impl Context {
    /// A scope the provider does not track never matches a binding: bindings
    /// carry a non-negative epoch, so `-1` refuses rather than authorizes. A
    /// binding like that is refused at issue, so reaching here means a record
    /// written before this provider tracked the scope.
    fn epoch_of(&self, scope: &str) -> i64 {
        self.epochs
            .iter()
            .find(|(name, _)| name == scope)
            .map_or(-1, |(_, epoch)| *epoch)
    }
}

impl Grant {
    pub fn key(id: &str) -> SubjectKey {
        SubjectKey {
            kind: KIND.to_string(),
            id: id.to_string(),
        }
    }

    /// The stored record, and what `core.grant.get` and the issue outcome
    /// return. Members are listed in the record's documented order; canonical
    /// form sorts them for storage and for any digest.
    pub fn to_value(&self) -> Value {
        let mut members = vec![
            ("id".into(), Value::String(self.id.clone())),
            ("issuer".into(), Value::String(self.issuer.clone())),
            ("holder".into(), Value::String(self.holder.clone())),
            ("audience".into(), Value::String(self.audience.clone())),
            (
                "rights".into(),
                Value::Array(self.rights.iter().cloned().map(Value::String).collect()),
            ),
            (
                "resources".into(),
                Value::Array(self.resources.iter().map(Resource::to_value).collect()),
            ),
        ];
        if let Some(instant) = &self.expires_at {
            members.push(("expires_at".into(), Value::String(instant.clone())));
        }
        if let Some(binding) = &self.authority_binding {
            members.push((
                "authority_binding".into(),
                Value::Object(vec![
                    ("scope".into(), Value::String(binding.scope.clone())),
                    ("epoch".into(), Value::Int(binding.epoch)),
                ]),
            ));
        }
        members.push((
            "delegation".into(),
            Value::Object(vec![
                ("allowed".into(), Value::Bool(self.delegation.allowed)),
                ("max_depth".into(), Value::Int(self.delegation.max_depth)),
            ]),
        ));
        if let Some(parent) = &self.parent {
            members.push(("parent".into(), Value::String(parent.clone())));
        }
        members.push((
            "state".into(),
            Value::String(if self.revoked { "revoked" } else { "active" }.into()),
        ));
        Value::Object(members)
    }

    /// Read a record back from the store. The store holds what `to_value`
    /// wrote, so a failure here is a corrupt store rather than a caller error.
    pub fn from_value(value: &Value) -> Option<Self> {
        let text = |name: &str| value.get(name).and_then(Value::as_str).map(str::to_string);
        Some(Grant {
            id: text("id")?,
            issuer: text("issuer")?,
            holder: text("holder")?,
            audience: text("audience")?,
            rights: value
                .get("rights")?
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            resources: value
                .get("resources")?
                .as_array()?
                .iter()
                .map(|item| Resource {
                    kind: item
                        .get("kind")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    id: item.get("id").and_then(Value::as_str).map(str::to_string),
                    id_prefix: item
                        .get("id_prefix")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                })
                .collect(),
            expires_at: text("expires_at"),
            authority_binding: value.get("authority_binding").and_then(|binding| {
                Some(Binding {
                    scope: binding.get("scope")?.as_str()?.to_string(),
                    epoch: match binding.get("epoch") {
                        Some(Value::Int(number)) => *number,
                        _ => return None,
                    },
                })
            }),
            delegation: Delegation {
                allowed: matches!(
                    value.get("delegation").and_then(|d| d.get("allowed")),
                    Some(Value::Bool(true))
                ),
                max_depth: match value.get("delegation").and_then(|d| d.get("max_depth")) {
                    Some(Value::Int(number)) => *number,
                    _ => 0,
                },
            },
            parent: text("parent"),
            revoked: text("state").as_deref() == Some("revoked"),
        })
    }

    /// Whether this grant is usable right now, ignoring rights and resources.
    ///
    /// Returned in CORE section 15.5's order, and used both when a caller acts
    /// under a grant and when a caller delegates from one, because a parent
    /// must be usable before its delegation rules are read at all.
    pub fn usable(&self, context: &Context) -> Result<(), Denial> {
        if self.revoked {
            return Err(Denial::Revoked);
        }
        if let Some(expiry) = &self.expires_at {
            // Instants are fixed-width UTC (`YYYY-MM-DDTHH:MM:SSZ`), so a
            // lexicographic comparison is a chronological one. A grant is
            // expired *from* its instant, so equality is already expired.
            if context.now.as_str() >= expiry.as_str() {
                return Err(Denial::Expired);
            }
        }
        if let Some(binding) = &self.authority_binding
            && context.epoch_of(&binding.scope) != binding.epoch
        {
            return Err(Denial::AuthorityEpochStale);
        }
        Ok(())
    }

    /// Whether this grant authorizes `needs`, each a right and the subject it
    /// is needed over. A need with no subject is a right the operation itself
    /// requires, such as `core.events.read`, whose per-subject filtering
    /// happens later against each event (CORE section 16.6).
    ///
    /// Every needed right is checked before any resource, because CORE section
    /// 15.5 reports `right_missing` ahead of `out_of_scope` even when the
    /// out-of-scope subject was named first.
    pub fn permits(&self, needs: &[Need]) -> Result<(), Denial> {
        for (right, _) in needs {
            if !self.rights.iter().any(|held| held == right) {
                return Err(Denial::RightMissing);
            }
        }
        for (_, subject) in needs {
            if let Some(subject) = subject
                && !self.resources.iter().any(|res| res.covers(subject))
            {
                return Err(Denial::OutOfScope);
            }
        }
        Ok(())
    }

    /// The delegation rules of CORE section 15.3: a child never exceeds its
    /// parent. Every violation is the one reason `delegation_exceeded`, so a
    /// delegate cannot map the parent's shape by reading which rule refused.
    ///
    /// The caller has already established that the parent is held by the
    /// issuer and is usable; this reads only the parent's delegation terms.
    pub fn may_delegate_to(&self, child: &Grant) -> Result<(), Denial> {
        if !self.delegation.allowed || self.delegation.max_depth < 1 {
            return Err(Denial::DelegationExceeded);
        }
        if child
            .rights
            .iter()
            .any(|right| !self.rights.contains(right))
        {
            return Err(Denial::DelegationExceeded);
        }
        if !child.resources.iter().all(|resource| {
            self.resources
                .iter()
                .any(|parent| parent.covers_resource(resource))
        }) {
            return Err(Denial::DelegationExceeded);
        }
        if let Some(parent_expiry) = &self.expires_at {
            match &child.expires_at {
                // A child with no expiry would outlive a parent that has one.
                None => return Err(Denial::DelegationExceeded),
                Some(child_expiry) if child_expiry > parent_expiry => {
                    return Err(Denial::DelegationExceeded);
                }
                Some(_) => {}
            }
        }
        if child.delegation.max_depth > self.delegation.max_depth - 1 {
            return Err(Denial::DelegationExceeded);
        }
        // A bound parent passes its binding down unchanged, so a takeover that
        // invalidates the parent invalidates everything delegated from it
        // without enumerating anything (CORE section 15.4).
        if self.authority_binding.is_some() && child.authority_binding != self.authority_binding {
            return Err(Denial::DelegationExceeded);
        }
        Ok(())
    }

    /// Whether any of this grant's resources covers a subject, with no right
    /// attached. `core.capabilities` is visible on exactly this basis, because
    /// no profile defines a read right for it (CORE section 16.6).
    pub fn covers(&self, subject: &SubjectKey) -> bool {
        self.resources.iter().any(|res| res.covers(subject))
    }

    /// Whether this grant covers a subject at all, for the read-disclosure
    /// rules: a current revision or a current epoch is shown only to a
    /// principal that could read that subject directly (CORE sections 15.5 and
    /// 16.6).
    pub fn may_read(&self, subject: &SubjectKey, right: &str) -> bool {
        self.rights.iter().any(|held| held == right)
            && self.resources.iter().any(|res| res.covers(subject))
    }
}

/// Read an issue payload into a record (CORE section 15.2).
///
/// Shape only: this is step 2 of the command path, so nothing here consults
/// the clock, the store or the principal. The validity checks that do — the
/// audience, the expiry and the binding scope — run at step 6 and live in the
/// provider, because CORE section 15.3 fixes their order relative to the
/// issuing rules and an already-bound command must replay past them.
pub fn parse_issue(id: &str, issuer: &str, payload: &Value) -> Result<Grant, ProtocolError> {
    let string = |name: &str| -> Result<String, ProtocolError> {
        match payload.get(name) {
            Some(Value::String(text)) if !text.is_empty() => Ok(text.clone()),
            _ => Err(ProtocolError::invalid_envelope(
                &format!("/payload/{name}"),
                "not a non-empty string",
            )),
        }
    };

    let rights = match payload.get("rights") {
        Some(Value::Array(items)) if !items.is_empty() => {
            let mut names = Vec::new();
            for (index, item) in items.iter().enumerate() {
                match item.as_str() {
                    Some(name) if !name.is_empty() => names.push(name.to_string()),
                    _ => {
                        return Err(ProtocolError::invalid_envelope(
                            &format!("/payload/rights/{index}"),
                            "not a right name",
                        ));
                    }
                }
            }
            names
        }
        _ => {
            return Err(ProtocolError::invalid_envelope(
                "/payload/rights",
                "not a non-empty array",
            ));
        }
    };

    let resources = match payload.get("resources") {
        Some(Value::Array(items)) if !items.is_empty() => {
            let mut parsed = Vec::new();
            for (index, item) in items.iter().enumerate() {
                let path = format!("/payload/resources/{index}");
                let Value::Object(members) = item else {
                    return Err(ProtocolError::invalid_envelope(&path, "not an object"));
                };
                for (name, _) in members {
                    if !matches!(name.as_str(), "kind" | "id" | "id_prefix") {
                        return Err(ProtocolError::invalid_envelope(
                            &format!("{path}/{name}"),
                            "unknown field",
                        ));
                    }
                }
                let kind = match item.get("kind").and_then(Value::as_str) {
                    Some(kind) if !kind.is_empty() => kind.to_string(),
                    _ => {
                        return Err(ProtocolError::invalid_envelope(
                            &format!("{path}/kind"),
                            "not a subject kind",
                        ));
                    }
                };
                let id = item.get("id").and_then(Value::as_str).map(str::to_string);
                let id_prefix = item
                    .get("id_prefix")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                // "optionally narrowed by exactly one of `id` or `id_prefix`":
                // both together would be two narrowings with no stated meaning.
                if id.is_some() && id_prefix.is_some() {
                    return Err(ProtocolError::invalid_envelope(
                        &path,
                        "id and id_prefix are exclusive",
                    ));
                }
                parsed.push(Resource {
                    kind,
                    id,
                    id_prefix,
                });
            }
            parsed
        }
        _ => {
            return Err(ProtocolError::invalid_envelope(
                "/payload/resources",
                "not a non-empty array",
            ));
        }
    };

    let delegation = match payload.get("delegation") {
        Some(delegation @ Value::Object(members)) => {
            for (name, _) in members {
                if !matches!(name.as_str(), "allowed" | "max_depth") {
                    return Err(ProtocolError::invalid_envelope(
                        &format!("/payload/delegation/{name}"),
                        "unknown field",
                    ));
                }
            }
            let allowed = match delegation.get("allowed") {
                Some(Value::Bool(flag)) => *flag,
                _ => {
                    return Err(ProtocolError::invalid_envelope(
                        "/payload/delegation/allowed",
                        "not a boolean",
                    ));
                }
            };
            let max_depth = match delegation.get("max_depth") {
                Some(Value::Int(depth)) if *depth >= 0 => *depth,
                _ => {
                    return Err(ProtocolError::invalid_envelope(
                        "/payload/delegation/max_depth",
                        "not a non-negative integer",
                    ));
                }
            };
            Delegation { allowed, max_depth }
        }
        _ => {
            return Err(ProtocolError::invalid_envelope(
                "/payload/delegation",
                "not an object",
            ));
        }
    };

    let expires_at = match payload.get("expires_at") {
        None => None,
        Some(Value::String(instant)) if is_instant(instant) => Some(instant.clone()),
        Some(_) => {
            return Err(ProtocolError::invalid_envelope(
                "/payload/expires_at",
                "not a UTC instant",
            ));
        }
    };

    let authority_binding = match payload.get("authority_binding") {
        None => None,
        Some(binding @ Value::Object(members)) => {
            for (name, _) in members {
                if !matches!(name.as_str(), "scope" | "epoch") {
                    return Err(ProtocolError::invalid_envelope(
                        &format!("/payload/authority_binding/{name}"),
                        "unknown field",
                    ));
                }
            }
            let scope = match binding.get("scope").and_then(Value::as_str) {
                Some(scope) if !scope.is_empty() => scope.to_string(),
                _ => {
                    return Err(ProtocolError::invalid_envelope(
                        "/payload/authority_binding/scope",
                        "not a scope name",
                    ));
                }
            };
            let epoch = match binding.get("epoch") {
                Some(Value::Int(epoch)) if *epoch >= 0 => *epoch,
                _ => {
                    return Err(ProtocolError::invalid_envelope(
                        "/payload/authority_binding/epoch",
                        "not a non-negative integer",
                    ));
                }
            };
            Some(Binding { scope, epoch })
        }
        Some(_) => {
            return Err(ProtocolError::invalid_envelope(
                "/payload/authority_binding",
                "not an object",
            ));
        }
    };

    // Constraint kinds are defined by profile features (CORE section 15.2).
    // This build implements none, and a constraint silently dropped would
    // widen the grant the issuer thought they were narrowing.
    if let Some(Value::Array(items)) = payload.get("constraints")
        && !items.is_empty()
    {
        return Err(ProtocolError::invalid_envelope(
            "/payload/constraints/0/kind",
            "no constraint kind is implemented",
        ));
    }

    let parent = match payload.get("parent") {
        None => None,
        Some(Value::String(id)) if crate::envelope::is_identifier(id) => Some(id.clone()),
        Some(_) => {
            return Err(ProtocolError::invalid_envelope(
                "/payload/parent",
                "not an identifier",
            ));
        }
    };

    Ok(Grant {
        id: id.to_string(),
        issuer: issuer.to_string(),
        holder: string("holder")?,
        audience: string("audience")?,
        rights,
        resources,
        expires_at,
        authority_binding,
        delegation,
        parent,
        revoked: false,
    })
}

/// `YYYY-MM-DDTHH:MM:SSZ` exactly, which is what makes a lexicographic
/// comparison of two instants a chronological one.
pub fn is_instant(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 20 {
        return false;
    }
    let digits = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    let punctuation = [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':')];
    digits.iter().all(|i| bytes[*i].is_ascii_digit())
        && punctuation.iter().all(|(i, c)| bytes[*i] == *c)
        && bytes[19] == b'Z'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(kind: &str, id: Option<&str>, prefix: Option<&str>) -> Resource {
        Resource {
            kind: kind.into(),
            id: id.map(str::to_string),
            id_prefix: prefix.map(str::to_string),
        }
    }

    fn subject(kind: &str, id: &str) -> SubjectKey {
        SubjectKey {
            kind: kind.into(),
            id: id.into(),
        }
    }

    #[test]
    fn a_prefix_resource_covers_only_its_own_kind() {
        let res = resource("core-test.subject", None, Some("s-"));
        assert!(res.covers(&subject("core-test.subject", "s-1")));
        assert!(!res.covers(&subject("core-test.subject", "t-1")));
        // The same id under a different kind is a different subject, and a
        // grant that ignored the kind would authorise the authority subject.
        assert!(!res.covers(&subject("core-test.authority", "s-1")));
    }

    #[test]
    fn a_missing_right_is_reported_before_a_subject_outside_the_grant() {
        // CORE section 15.5 fixes this order. The needs below name the
        // out-of-scope subject first, so an implementation that reported the
        // first failing need in order would answer `out_of_scope`.
        let grant = Grant {
            id: "g".into(),
            issuer: "owner".into(),
            holder: "agent".into(),
            audience: "p".into(),
            rights: vec!["core-test.read".into()],
            resources: vec![resource("core-test.subject", Some("s-1"), None)],
            expires_at: None,
            authority_binding: None,
            delegation: Delegation {
                allowed: false,
                max_depth: 0,
            },
            parent: None,
            revoked: false,
        };
        let needs = [
            ("core-test.read", Some(subject("core-test.subject", "s-2"))),
            ("core-test.write", Some(subject("core-test.subject", "s-1"))),
        ];
        assert_eq!(grant.permits(&needs), Err(Denial::RightMissing));
    }

    #[test]
    fn an_unnarrowed_child_resource_is_wider_than_a_prefixed_parent() {
        let parent = resource("core-test.subject", None, Some("s-"));
        assert!(parent.covers_resource(&resource("core-test.subject", Some("s-1"), None)));
        assert!(parent.covers_resource(&resource("core-test.subject", None, Some("s-a"))));
        assert!(!parent.covers_resource(&resource("core-test.subject", None, None)));
        assert!(!parent.covers_resource(&resource("core-test.subject", Some("t-1"), None)));
    }

    #[test]
    fn expiry_is_exclusive_of_its_own_instant_and_ranks_below_revocation() {
        let mut grant = Grant {
            id: "g".into(),
            issuer: "owner".into(),
            holder: "agent".into(),
            audience: "p".into(),
            rights: vec!["core-test.read".into()],
            resources: vec![resource("core-test.subject", None, None)],
            expires_at: Some("2030-01-02T00:00:00Z".into()),
            authority_binding: None,
            delegation: Delegation {
                allowed: false,
                max_depth: 0,
            },
            parent: None,
            revoked: false,
        };
        let at = |grant: &Grant, instant: &str| {
            grant.usable(&Context {
                now: instant.into(),
                epochs: Vec::new(),
            })
        };
        assert_eq!(at(&grant, "2030-01-01T23:59:59Z"), Ok(()));
        assert_eq!(at(&grant, "2030-01-02T00:00:00Z"), Err(Denial::Expired));
        // CORE section 15.5 reports revocation ahead of expiry.
        grant.revoked = true;
        assert_eq!(at(&grant, "2030-01-02T00:00:00Z"), Err(Denial::Revoked));
    }

    #[test]
    fn an_instant_must_be_exactly_the_fixed_utc_form() {
        assert!(is_instant("2030-01-02T00:00:00Z"));
        assert!(!is_instant("2030-01-02T00:00:00+00:00"));
        assert!(!is_instant("2030-01-02T00:00:00.000Z"));
        assert!(!is_instant("2030-1-2T0:0:0Z"));
    }
}
