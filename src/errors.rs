#[derive(Debug)]
pub enum WaveformError {
    DecreasingTimestamp {
        timestamp: u64,
    },
    InvalidId {
        id: usize,
    },
    InvalidWidth {
        id: usize,
        expected: usize,
        actual: usize,
    },
    MismatchedTimestamps,
}

pub type WaveformResult<T> = Result<T, WaveformError>;
