use thiserror::Error;


#[derive(Error, Debug)]
pub enum Connect4Error {

    #[error("Failed to evaluate position")]
    EvaluatePositionError,

    #[error("Worker thread join error")]
    WorkerThreadJoinError,
}

pub type Result<T> = std::result::Result<T, Connect4Error>;
