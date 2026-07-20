#[derive(Debug, Clone, Copy)]
pub(super) enum GitSetup {
    InitializeAndCommit,
    #[cfg(test)]
    Initialize,
}
