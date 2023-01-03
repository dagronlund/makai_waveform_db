use std::convert::TryInto;

use crate::history::WaveformHistory;

pub struct WaveformSignalReal {
    history: WaveformHistory,
    vectors: Vec<u8>,
    vector_index: usize,
}

impl WaveformSignalReal {
    pub fn new() -> Self {
        Self {
            history: WaveformHistory::new(),
            vectors: Vec::new(),
            vector_index: 0,
        }
    }

    pub fn get_history(&self) -> &WaveformHistory {
        &self.history
    }

    pub fn update(&mut self, timestamp_index: usize, value: f64) {
        self.history.add_change(timestamp_index, self.vector_index);
        self.vectors.append(&mut value.to_be_bytes().to_vec());
        self.vector_index += 1;
    }

    pub fn get_real(&self, index: usize) -> f64 {
        let range = (index * 8)..(index * 8) + 1;
        f64::from_be_bytes((&self.vectors[range]).try_into().unwrap())
    }

    pub fn get_vector_size(&self) -> usize {
        self.vectors.len()
    }

    pub fn get_width(&self) -> usize {
        64
    }

    pub fn len(&self) -> usize {
        self.vector_index
    }

    pub fn is_empty(&self) -> bool {
        self.vector_index == 0
    }
}

impl Default for WaveformSignalReal {
    fn default() -> Self {
        Self::new()
    }
}
