//! What a launch does, decided from its arguments and its configuration
//! alone.
//!
//! # Why this is a function rather than an order of statements
//!
//! "A careless launch returns before the credential is read" used to be a
//! property of **where the checks sat in `main`**, and the only thing
//! testing it was a set of process-level tests asserting that the message
//! did not mention a keychain. That assertion would not have noticed a
//! *successful, silent* read — which on the owner's own machine is the
//! failure that matters, because it is their real key.
//!
//! So the decision is taken here, purely, and every row of it is tested
//! without starting a process. Whether a credential is read is then a
//! property of the [`Decision`] rather than of the code that acts on it,
//! and [`tests::a_credential_is_read_only_when_a_model_is_configured_and_permitted`]
//! states it over every input rather than over a table somebody remembered
//! to extend.

/// Whether anything a *serving* launch does can reach a model.
///
/// **True since m4c connected selection to the transport.** A serving
/// provider now makes a model call while preparing a request that
/// authorised an investigation: `provider::Serving::ask`, reached from
/// `select_source` by way of the work pool, choosing which of the ranked
/// spans to cite. So a launch given a model, the permit and a credential
/// has somewhere to spend it, and reading one is no longer *a prompt, an
/// audit entry and a secret in a process that had no use for one*
/// ([READINESS §2](../../docs/work/m4/READINESS.md)).
///
/// **What changed with it, and what did not.** While this was false the
/// calibration was the only launch that read a credential at all, and the
/// tests holding that were written as implications so they would stop
/// constraining the row rather than have to be found and deleted. They
/// have stopped. What has not changed is that **no test in the suite
/// launches a serving provider with a real model**: the one test that
/// passes `--permit-model-network` with a valid configuration asks for
/// the calibration and is gated off macOS, and
/// `tests/credential_discipline.rs` now holds that rule over the sources
/// rather than over anybody's memory of it.
pub const SERVING_CALLS_A_MODEL: bool = true;

/// Everything about a launch that the decision depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Launch {
    /// A `model_runtime` member was present and valid.
    pub model_configured: bool,
    pub permit_model_network: bool,
    pub calibrate: bool,
    pub model_run_ceiling: Option<u64>,
}

/// What the launch does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Refuse to start, with a reason for the operator.
    Refuse(String),
    /// Serve, with no model and **no credential read**.
    Serve,
    /// Read the credential once, then serve.
    ReadCredentialThenServe,
    /// Read the credential once, run the calibration, and exit.
    ReadCredentialThenCalibrate,
}

impl Decision {
    /// Whether this decision touches the Keychain.
    pub fn reads_credential(&self) -> bool {
        matches!(
            self,
            Decision::ReadCredentialThenServe | Decision::ReadCredentialThenCalibrate
        )
    }
}

pub fn decide(launch: &Launch) -> Decision {
    let refuse = |reason: String| Decision::Refuse(reason);
    // **The calibration's own refusals come first**, so an operator who
    // asked for it is told about the thing they asked for. Every one of
    // them is still a refusal, and a refusal reads no credential, so the
    // order changes the message and not the safety — which is the kind of
    // question that was invisible while this was a sequence of statements
    // in `main`.
    if launch.calibrate {
        if !launch.model_configured {
            return refuse("--calibrate needs a configured model".into());
        }
        if !launch.permit_model_network {
            return refuse(
                "--calibrate makes live calls and needs --permit-model-network; having built \
                 a transport is not permission to use it"
                    .into(),
            );
        }
        match launch.model_run_ceiling {
            None => {
                return refuse(format!(
                    "--calibrate needs --model-run-ceiling; the cap of {} tokens is enforced \
                     by the ledger rather than by intention",
                    crate::calibration::CEILING
                ));
            }
            Some(ceiling) if ceiling > crate::calibration::CEILING => {
                return refuse(format!(
                    "--model-run-ceiling {ceiling} is above the calibration's cap of {}",
                    crate::calibration::CEILING
                ));
            }
            Some(_) => {}
        }
        return Decision::ReadCredentialThenCalibrate;
    }
    // **The two go together in both directions.** Permitting calls with no
    // model configured is a configuration mistake rather than a safe
    // default; a configured model without the flag would serve while
    // failing every model call. Refusing both is what makes a credential
    // read impossible without a deliberate act, which is in turn what
    // keeps the owner's key out of every test run.
    if launch.permit_model_network && !launch.model_configured {
        return refuse(
            "--permit-model-network needs a configured model; permitting calls to nothing \
             is a configuration mistake rather than a safe default"
                .into(),
        );
    }
    if launch.model_configured {
        if !launch.permit_model_network {
            return refuse(
                "a model is configured and --permit-model-network was not given; a process \
                 that would fail every model call is refused rather than started"
                    .into(),
            );
        }
        if !SERVING_CALLS_A_MODEL {
            return refuse(
                "a model is configured and this build has no call site to spend it at: \
                 serving would read the credential and never use it. Until the model \
                 runtime is connected, `--calibrate` is the only launch that may read one"
                    .into(),
            );
        }
        return Decision::ReadCredentialThenServe;
    }
    Decision::Serve
}

#[cfg(test)]
mod tests;
