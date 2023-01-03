// Coming from a VCD file there are two primary and slightly opposed ways to
// represent the waveform in-memory.
// 1. A compressed but complete format where the minimum amount of memory is
//    used to store the waveforms. This format needs to be easily queried for
//    the waveform value at arbitrary timestamps, as well as knowing if any
//    transitions exist within a given timespan.
// 2. A format that allows the waveform to be shown on the display for a given
//    timespan. This format cannot simply be downsampled from the compressed
//    format because that would miss brief transitions that still need to be
//    shown in the waveform. This format has a finite resolution given to it
//    before it is extracted from the compressed waveform.

// Compressed Format:
//    A critical part of the compressed format is that it can be easily searched
//    like a binary tree for the desired timestamp index. This requires
//    fixed-size blocks of data with headers. These fixed-size blocks start with
//    a header containing the 64-bit timestamp offset of the first change/skip
//    in the block. The header also contains another 64-bit mask representing
//    whether each of the 64 locations in the block is a header or a skip count.
//
//    The block size is fixed at 512 bytes, where the first 16 bytes store the
//    timestamp and then then bit-vector offset, and all following bytes either
//    indicate transitions or timestamp skips.
//
//    By looking at the first byte in this list, it's MSB is a one if it
//    represents a number of consecutive changes (from 1 to 128) in the value.
//    If the MSB is a zero, then it and up to eight following bytes with an
//    MSB of zero are a skip count (up to 64 bits excluding the MSBs). Skips at
//    the end of a block are ignored since the next block is going to define a
//    new starting timestamp.
//
//    The compressed data for each signal in the waveform references timestamp
//    indices, not the timestamps themselves.

pub mod bitvector;
pub mod errors;
pub mod history;
pub mod real;
pub mod vector;

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::bitvector::BitVector;
use crate::errors::*;
use crate::real::*;
use crate::vector::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WaveformSearchMode {
    Before,
    After,
    Closest,
    Exact,
}

pub enum WaveformSignalResult<'a> {
    Vector(&'a WaveformSignalVector),
    Real(&'a WaveformSignalReal),
}

#[derive(Clone, Debug, PartialEq)]
pub enum WaveformValueResult {
    Vector(BitVector, usize), // value, timestamp index
    Real(f64, usize),         // value, timestamp index
}

impl WaveformValueResult {
    pub fn is_unknown(&self) -> bool {
        match self {
            Self::Vector(bv, _) => bv.is_unknown(),
            _ => false,
        }
    }

    pub fn is_high_impedance(&self) -> bool {
        match self {
            Self::Vector(bv, _) => bv.is_high_impedance(),
            _ => false,
        }
    }

    pub fn get_timestamp_index(&self) -> usize {
        match self {
            Self::Vector(_, index) | Self::Real(_, index) => *index,
        }
    }
}

pub struct Waveform {
    timestamps: Vec<u64>,
    vector_signals: HashMap<usize, WaveformSignalVector>,
    real_signals: HashMap<usize, WaveformSignalReal>,
}

impl Waveform {
    pub fn new() -> Self {
        Self {
            timestamps: Vec::new(),
            vector_signals: HashMap::default(),
            real_signals: HashMap::default(),
        }
    }

    pub fn shard(self, num_shards: usize) -> Vec<Self> {
        let mut shards = Vec::new();
        for _ in 0..num_shards {
            let mut shard = Self::new();
            shard.timestamps = self.timestamps.clone();
            shards.push(shard);
        }
        for (id, signal) in self.vector_signals {
            shards[id % num_shards].vector_signals.insert(id, signal);
        }
        for (id, signal) in self.real_signals {
            shards[id % num_shards].real_signals.insert(id, signal);
        }
        shards
    }

    pub fn unshard(shards: Vec<Self>) -> WaveformResult<Self> {
        let timestamps = if let Some(shard) = shards.first() {
            shard.timestamps.clone()
        } else {
            Vec::new()
        };
        for shard in &shards {
            if shard.timestamps != timestamps {
                return Err(WaveformError::MismatchedTimestamps);
            }
        }
        let mut merged = Self::new();
        merged.timestamps = timestamps;
        for shard in shards {
            merged.vector_signals.extend(shard.vector_signals);
            merged.real_signals.extend(shard.real_signals);
        }
        Ok(merged)
    }

    pub fn initialize_vector(&mut self, id: usize, width: usize) {
        self.vector_signals
            .insert(id, WaveformSignalVector::new(width));
    }

    pub fn initialize_real(&mut self, id: usize) {
        self.real_signals.insert(id, WaveformSignalReal::new());
    }

    pub fn get_vector_signal(&self, id: usize) -> Option<&WaveformSignalVector> {
        self.vector_signals.get(&id)
    }

    pub fn get_real_signal(&self, id: usize) -> Option<&WaveformSignalReal> {
        self.real_signals.get(&id)
    }

    pub fn get_signal(&self, id: usize) -> Option<WaveformSignalResult<'_>> {
        if let Some(signal) = self.vector_signals.get(&id) {
            Some(WaveformSignalResult::Vector(signal))
        } else if let Some(signal) = self.real_signals.get(&id) {
            Some(WaveformSignalResult::Real(signal))
        } else {
            None
        }
    }

    pub fn get_timestamps(&self) -> &Vec<u64> {
        &self.timestamps
    }

    pub fn insert_timestamp(&mut self, timestamp: u64) -> WaveformResult<()> {
        let Some(last) = self.timestamps.last() else {
            self.timestamps.push(timestamp);
            return Ok(());
        };
        match timestamp.cmp(last) {
            Ordering::Less => return Err(WaveformError::DecreasingTimestamp { timestamp }),
            Ordering::Greater => self.timestamps.push(timestamp),
            Ordering::Equal => {}
        }
        Ok(())
    }

    pub fn update_vector(&mut self, id: usize, value: BitVector) -> WaveformResult<()> {
        let signal = if let Some(signal) = self.vector_signals.get_mut(&id) {
            signal
        } else {
            return Err(WaveformError::InvalidId { id });
        };
        if signal.get_width() < value.get_bit_width() {
            return Err(WaveformError::InvalidWidth {
                id,
                expected: signal.get_width(),
                actual: value.get_bit_width(),
            });
        }
        signal.update(self.timestamps.len() - 1, value);
        Ok(())
    }

    pub fn update_real(&mut self, id: usize, value: f64) -> WaveformResult<()> {
        let signal = if let Some(signal) = self.real_signals.get_mut(&id) {
            signal
        } else {
            return Err(WaveformError::InvalidId { id });
        };
        signal.update(self.timestamps.len() - 1, value);
        Ok(())
    }

    pub fn timestamps_count(&self) -> usize {
        self.timestamps.len()
    }

    pub fn get_block_size(&self) -> usize {
        let mut size = 0;
        for signal in self.vector_signals.values() {
            size += signal.get_history().get_block_size();
        }
        for signal in self.real_signals.values() {
            size += signal.get_history().get_block_size();
        }
        size
    }

    pub fn get_vector_size(&self) -> usize {
        let mut size = 0;
        for signal in self.vector_signals.values() {
            size += signal.get_vector_size();
        }
        for signal in self.real_signals.values() {
            size += signal.get_vector_size();
        }
        size
    }

    pub fn count_empty(&self) -> usize {
        let mut empty = 0;
        for signal in self.vector_signals.values() {
            if signal.is_empty() {
                empty += 1;
            }
        }
        for signal in self.real_signals.values() {
            if signal.is_empty() {
                empty += 1;
            }
        }
        empty
    }

    pub fn count_one(&self) -> usize {
        let mut empty = 0;
        for signal in self.vector_signals.values() {
            if signal.len() == 1 {
                empty += 1;
            }
        }
        for signal in self.real_signals.values() {
            if signal.len() == 1 {
                empty += 1;
            }
        }
        empty
    }

    /// Returns the first and last timestamps (both inclusive) that are present
    /// in this waveform
    pub fn get_timestamp_range(&self) -> std::ops::Range<u64> {
        match (self.get_timestamps().first(), self.get_timestamps().last()) {
            (Some(start), Some(end)) => *start..*end,
            _ => 0..0,
        }
    }

    /// Binary search for the index of the requested timestamp, if the exact
    /// timestamp exists. Otherwise, the search mode is used to determine where
    /// else to look to look for a timestamp, either closest of any timestamp,
    /// closest timestamp before, or closest timestamp after. If a timestamp
    /// is found, this function returns a tuple (timestamp, timestamp index)
    pub fn search_timestamp(
        &self,
        timestamp: u64,
        search_mode: WaveformSearchMode,
    ) -> Option<usize> {
        // https://stackoverflow.com/questions/30245166/find-the-nearest-closest-value-in-a-sorted-list
        let (mut start, mut end) = (0, self.timestamps.len() - 1);
        // If the search timestamp is outside of the range of timestamps
        if timestamp < self.timestamps[start] {
            return match search_mode {
                WaveformSearchMode::Exact | WaveformSearchMode::Before => None,
                WaveformSearchMode::After | WaveformSearchMode::Closest => Some(start),
            };
        } else if self.timestamps[end] < timestamp {
            return match search_mode {
                WaveformSearchMode::Exact | WaveformSearchMode::After => None,
                WaveformSearchMode::Before | WaveformSearchMode::Closest => Some(end),
            };
        }
        // Iterate through until start == end + 1
        while start <= end {
            let mid = (start + end) / 2;
            let mid_value = self.timestamps[mid];
            match timestamp.cmp(&mid_value) {
                Ordering::Less => end = mid - 1,
                Ordering::Greater => start = mid + 1,
                Ordering::Equal => return Some(mid),
            }
        }
        // Select result based on search mode
        match search_mode {
            WaveformSearchMode::Exact => None,
            WaveformSearchMode::Before => Some(end),
            WaveformSearchMode::After => Some(start),
            WaveformSearchMode::Closest => {
                if (self.timestamps[start] - timestamp) < (timestamp - self.timestamps[end]) {
                    Some(start)
                } else {
                    Some(end)
                }
            }
        }
    }

    pub fn search_value_bit_index(
        &self,
        idcode: usize,
        timestamp_index: usize,
        search_mode: WaveformSearchMode,
        bit_index: Option<usize>,
    ) -> Option<WaveformValueResult> {
        match self.get_signal(idcode) {
            Some(WaveformSignalResult::Vector(signal)) => {
                let Some(index) = signal.get_history().search_timestamp_index(timestamp_index, search_mode) else {
                    return None
                };
                let bv = signal.get_bitvector(index.get_value_index());
                let bv = if let Some(index) = bit_index {
                    BitVector::from(bv.get_bit(index))
                } else {
                    bv
                };
                Some(WaveformValueResult::Vector(bv, index.get_timestamp_index()))
            }
            Some(WaveformSignalResult::Real(signal)) => {
                let Some(index) = signal.get_history().search_timestamp_index(timestamp_index, search_mode) else {
                    return None
                };
                let r = signal.get_real(index.get_value_index());
                Some(WaveformValueResult::Real(r, index.get_timestamp_index()))
            }
            None => None,
        }
    }

    pub fn search_value(
        &self,
        idcode: usize,
        timestamp_index: usize,
        search_mode: WaveformSearchMode,
    ) -> Option<WaveformValueResult> {
        self.search_value_bit_index(idcode, timestamp_index, search_mode, None)
    }
}

impl Default for Waveform {
    fn default() -> Self {
        Self::new()
    }
}
