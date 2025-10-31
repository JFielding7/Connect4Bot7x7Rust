use std::io;
use thiserror::Error;


#[derive(Error, Debug)]
pub enum Connect4Error {

    #[error("Failed to evaluate position")]
    EvaluatePositionError,

    #[error("Worker thread join error")]
    WorkerThreadJoinError,

    #[error("{0}")]
    DatabaseIOError(#[from] io::Error)
}

pub type Result<T> = std::result::Result<T, Connect4Error>;
