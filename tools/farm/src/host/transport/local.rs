//! This machine, through `sh`.

use crate::host::transport::process::Failure;
use crate::host::transport::program::Call;
use crate::host::transport::{Done, Exec, Pull, Push, Transport};

/// This machine: a call is `sh -c <program>`, a workspace is where it is.
#[derive(Debug, Clone)]
pub(crate) struct Local {
    exec: Exec,
}

impl Local {
    pub(crate) fn new(exec: Exec) -> Self {
        Self { exec }
    }
}

impl Transport for Local {
    fn call(&self, call: &Call, lines: &mut impl FnMut(&str)) -> Result<Done, Failure> {
        self.exec.spawn(&call.argv(), call.into(), lines)
    }

    fn push(&self, _: &Push<'_>) -> Result<(), Failure> {
        Ok(())
    }

    fn pull(&self, _: &Pull<'_>) -> Result<(), Failure> {
        Ok(())
    }

    fn is_local(&self) -> bool {
        true
    }
}
