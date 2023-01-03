#[derive(Clone, Debug, PartialEq)]
pub struct WaveformHistoryIndex {
    pub timestamp_index: usize,
    pub value_index: usize,
}

impl WaveformHistoryIndex {
    pub fn get_timestamp_index(&self) -> usize {
        self.timestamp_index
    }

    pub fn get_value_index(&self) -> usize {
        self.value_index
    }
}
