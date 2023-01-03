pub mod block;
pub mod index;

use std::cmp::Ordering;

use crate::history::block::WaveformHistoryBlock;
use crate::history::block::WaveformHistoryBlockIter;
use crate::history::index::WaveformHistoryIndex;
use crate::WaveformSearchMode;

pub const BLOCK_SIZE: usize = 512;
pub const MAX_BLOCK_CHANGES: usize = BLOCK_SIZE * 128;

pub struct WaveformHistory {
    timestamp_index_last: isize,
    blocks: Vec<u8>,
    block_index: isize,
    block_offset: usize,
}

impl WaveformHistory {
    pub fn new() -> Self {
        Self {
            timestamp_index_last: -1,
            blocks: Vec::new(),
            block_index: -1,
            block_offset: 16,
        }
    }

    // Returns true if there was room in the block for the requested skip and
    // extra byte for a change after it, because if there isn't then it a new
    // block has to be used
    fn insert_change_block(&mut self, timestamp_index_diff: usize) -> bool {
        let skips = timestamp_index_diff - 1;
        let block_index = self.block_index as usize;
        if skips > 0 {
            let skip_bits = usize::BITS - skips.leading_zeros();
            let skip_bytes = ((skip_bits - 1) / 7 + 1) as usize;
            // We need space for the skip bytes plus one change byte
            if (BLOCK_SIZE - self.block_offset) < (skip_bytes + 1) {
                return false;
            }
            // Insert skip bytes
            let mut skips = skips;
            for i in (0..skip_bytes).rev() {
                self.blocks[block_index * BLOCK_SIZE + self.block_offset + i] = (skips & 127) as u8;
                skips >>= 7;
            }
            self.block_offset += skip_bytes;
            // Insert change byte
            self.blocks[block_index * BLOCK_SIZE + self.block_offset] = 128;
            self.block_offset += 1;
            true
        } else {
            let index = block_index * BLOCK_SIZE + self.block_offset;
            // Check if the last change byte has remaining space
            if self.blocks[index - 1] < 255 {
                self.blocks[index - 1] += 1;
                true
            } else {
                // Check if we have remaining bytes for a new change byte
                if self.block_offset < BLOCK_SIZE {
                    self.blocks[index] = 128;
                    self.block_offset += 1;
                    return true;
                }
                false
            }
        }
    }

    fn insert_block(&mut self, timestamp_index: usize, value_index: usize) {
        self.blocks.append(&mut vec![0; BLOCK_SIZE]);
        self.block_index += 1;
        // Write timestamp and value index for this block
        let block_bytes = self.block_index as usize * BLOCK_SIZE;
        self.blocks[block_bytes..block_bytes + 8]
            .clone_from_slice(&(timestamp_index as u64).to_be_bytes());
        self.blocks[block_bytes + 8..block_bytes + 16]
            .clone_from_slice(&(value_index as u64).to_be_bytes());
        // Write a single byte indicating one change occurred
        self.blocks[block_bytes + 16] = 128;
        self.block_offset = 17;
    }

    pub fn add_change(&mut self, timestamp_index: usize, value_index: usize) {
        if self.timestamp_index_last >= 0 {
            let timestamp_index_last = self.timestamp_index_last as usize;
            if timestamp_index == timestamp_index_last {
                panic!(
                    "TODO: Timestamp index duplicated! {} == {}",
                    timestamp_index, timestamp_index_last
                );
            }
            // Add another block if the change insertion fails
            if !self.insert_change_block(timestamp_index - timestamp_index_last) {
                self.insert_block(timestamp_index, value_index);
            }
        } else {
            // Insert block since we have none currently
            self.insert_block(timestamp_index, value_index);
        }

        self.timestamp_index_last = timestamp_index as isize;
    }

    pub fn get_block(&self, block_index: usize) -> WaveformHistoryBlock {
        WaveformHistoryBlock::new(
            &self.blocks[(block_index * BLOCK_SIZE)..((block_index + 1) * BLOCK_SIZE)],
        )
    }

    pub fn get_block_count(&self) -> usize {
        self.blocks.len() / BLOCK_SIZE
    }

    pub fn get_block_size(&self) -> usize {
        self.blocks.len()
    }

    fn search_timestamp_block_index(
        &self,
        timestamp_index: usize,
        search_mode: WaveformSearchMode,
    ) -> Option<usize> {
        // https://stackoverflow.com/questions/30245166/find-the-nearest-closest-value-in-a-sorted-list
        let (mut start, mut end) = (0, self.get_block_count() - 1);
        // If the search timestamp is outside of the range of timestamps
        if timestamp_index < self.get_block(start).get_timestamp_index() {
            return match search_mode {
                WaveformSearchMode::Exact | WaveformSearchMode::Before => None,
                WaveformSearchMode::After | WaveformSearchMode::Closest => Some(start),
            };
        } else if self.get_block(end).get_timestamp_index() + MAX_BLOCK_CHANGES < timestamp_index {
            return match search_mode {
                WaveformSearchMode::Exact | WaveformSearchMode::After => None,
                WaveformSearchMode::Before | WaveformSearchMode::Closest => Some(end),
            };
        }
        // Iterate through until start == end + 1
        while start <= end {
            let mid = (start + end) / 2;
            let mid_value = self.get_block(mid).get_timestamp_index();
            match timestamp_index.cmp(&mid_value) {
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
                if (self.get_block(start).get_timestamp_index() - timestamp_index)
                    < (timestamp_index - self.get_block(end).get_timestamp_index())
                {
                    Some(start)
                } else {
                    Some(end)
                }
            }
        }
    }

    /// Returns the waveform index for the change at the given timestamp index
    pub fn search_timestamp_index(
        &self,
        timestamp_index: usize,
        search_mode: WaveformSearchMode,
    ) -> Option<WaveformHistoryIndex> {
        let Some(block_index) = self.search_timestamp_block_index(
            timestamp_index, WaveformSearchMode::Before
        ) else {
            return None
        };
        // Determine if there are changes before the timestamp
        let mut iter = self.get_block(block_index).into_iter();
        let Some(index_before) = iter.seek(timestamp_index) else {
            // No timestamp is before the given timestamp
            return match search_mode {
                WaveformSearchMode::After | WaveformSearchMode::Closest =>
                        self.get_block(0).into_iter().next(),
                _ => None,
            };
        };
        // Check for exact solution first before extra work
        if index_before.get_timestamp_index() == timestamp_index {
            return Some(index_before);
        }
        // Check if there are changes after the timestamp
        let index_after = if let Some(index_after) = iter.next() {
            Some(index_after)
        } else if block_index + 1 < self.get_block_count() {
            self.get_block(block_index + 1).into_iter().next()
        } else {
            None
        };
        // Calculate result from search mode
        match (search_mode, index_after) {
            (WaveformSearchMode::Before, _) => Some(index_before),
            (WaveformSearchMode::After, Some(index_after)) => Some(index_after),
            (WaveformSearchMode::After, None) => None,
            (WaveformSearchMode::Closest, Some(index_after)) => {
                if (index_after.get_timestamp_index() - timestamp_index)
                    < (timestamp_index - index_before.get_timestamp_index())
                {
                    Some(index_after)
                } else {
                    Some(index_before)
                }
            }
            (WaveformSearchMode::Closest, None) => Some(index_before),
            (WaveformSearchMode::Exact, _) => None,
        }
    }
}

impl Default for WaveformHistory {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WaveformHistoryIter<'a> {
    block_index: usize,
    block_iter: WaveformHistoryBlockIter<'a>,
    history: &'a WaveformHistory,
}

impl<'a> Iterator for WaveformHistoryIter<'a> {
    type Item = WaveformHistoryIndex;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.block_index >= self.history.get_block_count() {
                return None;
            } else if let Some(index) = self.block_iter.next() {
                return Some(index);
            }
            // Go to next block if nothing found in last one
            self.next_block();
        }
    }
}

impl<'a> IntoIterator for &'a WaveformHistory {
    type Item = WaveformHistoryIndex;

    type IntoIter = WaveformHistoryIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        WaveformHistoryIter {
            block_index: 0,
            block_iter: self.get_block(0).into_iter(),
            history: self,
        }
    }
}

impl<'a> WaveformHistoryIter<'a> {
    fn next_block(&mut self) {
        self.block_index += 1;
        if self.block_index < self.history.get_block_count() {
            self.block_iter = self.history.get_block(self.block_index).into_iter();
        }
    }

    pub fn seek(&mut self, timestamp_index: usize) -> Option<WaveformHistoryIndex> {
        let mut last_block_index = self.block_index;
        let mut last_block_iter = self.block_iter.clone();
        let mut last_index = None;
        loop {
            if self.block_index >= self.history.get_block_count() {
                self.block_index = last_block_index;
                self.block_iter = last_block_iter;
                return last_index;
            }
            // Check if the current block has a result
            if let Some(index) = self.block_iter.seek(timestamp_index) {
                last_block_index = self.block_index;
                last_block_iter = self.block_iter.clone();
                last_index = Some(index);
            } else {
                self.block_index = last_block_index;
                self.block_iter = last_block_iter;
                return last_index;
            }
            // Go to next block if nothing found in last one
            self.next_block();
        }
    }
}
