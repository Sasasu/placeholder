use std::io::Error as IoError;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum Error {
    IoError(IoError),
    RecvError(mpsc::error::UnboundedRecvError),
}

impl From<mpsc::error::UnboundedRecvError> for Error {
    fn from(e: mpsc::error::UnboundedRecvError) -> Self {
        Error::RecvError(e)
    }
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Self {
        Error::IoError(e)
    }
}
