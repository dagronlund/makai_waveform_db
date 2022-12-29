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
pub mod signal_history;
pub mod signal_real;
pub mod signal_vector;

use std::collections::HashMap;

use crate::bitvector::BitVector;
use crate::errors::*;
use crate::signal_real::*;
use crate::signal_vector::*;

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

    pub fn get_signal<'a>(&'a self, id: usize) -> Option<WaveformSignalResult<'a>> {
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
        if let Some(last) = self.timestamps.last() {
            if timestamp < *last {
                return Err(WaveformError::DecreasingTimestamp {
                    timestamp: timestamp,
                });
            } else if timestamp > *last {
                self.timestamps.push(timestamp);
            }
        } else {
            self.timestamps.push(timestamp);
        }
        Ok(())
    }

    pub fn update_vector(&mut self, id: usize, value: BitVector) -> WaveformResult<()> {
        let signal = if let Some(signal) = self.vector_signals.get_mut(&id) {
            signal
        } else {
            return Err(WaveformError::InvalidId { id: id });
        };
        if signal.get_width() < value.get_bit_width() {
            return Err(WaveformError::InvalidWidth {
                id: id,
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
            return Err(WaveformError::InvalidId { id: id });
        };
        signal.update(self.timestamps.len() - 1, value);
        Ok(())
    }

    pub fn timestamps_count(&self) -> usize {
        self.timestamps.len()
    }

    pub fn get_block_size(&self) -> usize {
        let mut size = 0;
        for (_, signal) in &self.vector_signals {
            size += signal.get_history().get_block_size();
        }
        for (_, signal) in &self.real_signals {
            size += signal.get_history().get_block_size();
        }
        size
    }

    pub fn get_vector_size(&self) -> usize {
        let mut size = 0;
        for (_, signal) in &self.vector_signals {
            size += signal.get_vector_size();
        }
        for (_, signal) in &self.real_signals {
            size += signal.get_vector_size();
        }
        size
    }

    pub fn count_empty(&self) -> usize {
        let mut empty = 0;
        for (_, signal) in &self.vector_signals {
            if signal.len() == 0 {
                empty += 1;
            }
        }
        for (_, signal) in &self.real_signals {
            if signal.len() == 0 {
                empty += 1;
            }
        }
        empty
    }

    pub fn count_one(&self) -> usize {
        let mut empty = 0;
        for (_, signal) in &self.vector_signals {
            if signal.len() == 1 {
                empty += 1;
            }
        }
        for (_, signal) in &self.real_signals {
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

    fn search_timestamp_recursive(
        &self,
        timestamp: u64,
        range: std::ops::Range<usize>,
        after: bool,
    ) -> Option<usize> {
        let not_found = if after {
            self.timestamps[range.end - 1] < timestamp
        } else {
            timestamp < self.timestamps[range.start]
        };
        if not_found {
            None
        } else if range.len() > 1 {
            let mid = (range.start + range.end) / 2;
            let look_before = if after {
                timestamp <= self.timestamps[mid - 1]
            } else {
                timestamp < self.timestamps[mid]
            };
            if look_before {
                self.search_timestamp_recursive(timestamp, range.start..mid, after)
            } else {
                self.search_timestamp_recursive(timestamp, mid..range.end, after)
            }
        } else {
            Some(range.start)
        }
    }

    /// Binary search for the index of the requested timestamp, or if not
    /// found the timestamp immediately before it
    pub fn search_timestamp(&self, timestamp: u64) -> Option<usize> {
        self.search_timestamp_recursive(timestamp, 0..self.timestamps.len(), false)
    }

    /// Binary search for the index of the requested timestamp, or if not
    /// found the timestamp immediately after it
    pub fn search_timestamp_after(&self, timestamp: u64) -> Option<usize> {
        self.search_timestamp_recursive(timestamp, 0..self.timestamps.len(), true)
    }

    /// Returns the range of timestamp indices that either contains the given
    /// timestamps (greedy) or is contained by the given timestamps (non-greedy)
    pub fn search_timestamp_range(
        &self,
        timestamp_range: std::ops::Range<u64>,
        greedy: bool,
    ) -> Option<std::ops::Range<usize>> {
        // First find the non-greedy bounds
        let mut start = self.search_timestamp_after(timestamp_range.start);
        let mut end = self.search_timestamp(timestamp_range.end);
        if greedy {
            // If the greedy bounds exist, use them
            if let Some(s) = self.search_timestamp(timestamp_range.start) {
                start = Some(s);
            }
            if let Some(e) = self.search_timestamp_after(timestamp_range.end) {
                end = Some(e);
            }
        } else {
            // If not greedy then make sure the end index is not at the same
            // timestamp as the timestamp range end, otherwise step back one
            if let Some(e) = end {
                if self.timestamps[e] == timestamp_range.end {
                    if let Some(e) = self.search_timestamp(timestamp_range.end - 1) {
                        end = Some(e);
                    }
                }
            }
        }
        if let (Some(start), Some(end)) = (start, end) {
            Some(start..end)
        } else {
            None
        }
    }

    pub fn search_value_bit_index(
        &self,
        idcode: usize,
        timestamp_index: usize,
        bit_index: Option<usize>,
    ) -> Option<WaveformValueResult> {
        match self.get_signal(idcode) {
            Some(WaveformSignalResult::Vector(signal)) => {
                let Some(pos) = signal.get_history().search_timestamp_index(timestamp_index) else {
                    return None
                };
                let pos = pos.get_index();
                let bv = signal.get_bitvector(pos.get_value_index());
                let bv = if let Some(index) = bit_index {
                    BitVector::from(bv.get_bit(index))
                } else {
                    bv
                };
                Some(WaveformValueResult::Vector(bv, pos.get_timestamp_index()))
            }
            Some(WaveformSignalResult::Real(signal)) => {
                let Some(pos) = signal.get_history().search_timestamp_index(timestamp_index) else {
                    return None
                };
                let pos = pos.get_index();
                let r = signal.get_real(pos.get_value_index());
                Some(WaveformValueResult::Real(r, pos.get_timestamp_index()))
            }
            None => None,
        }
    }

    pub fn search_value(
        &self,
        idcode: usize,
        timestamp_index: usize,
    ) -> Option<WaveformValueResult> {
        self.search_value_bit_index(idcode, timestamp_index, None)
    }
}

impl Default for Waveform {
    fn default() -> Self {
        Self::new()
    }
}
