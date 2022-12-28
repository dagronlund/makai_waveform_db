use std::convert::TryInto;
use std::ops::Range;

const BLOCK_SIZE: usize = 512;

pub struct WaveformSignalHistory {
    timestamp_index_last: isize,
    blocks: Vec<u8>,
    block_index: isize,
    block_offset: usize,
}

impl WaveformSignalHistory {
    pub(crate) fn new() -> Self {
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
    fn insert_change(&mut self, timestamp_index_diff: usize) -> bool {
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

    pub(crate) fn update_block(&mut self, timestamp_index: usize, value_index: usize) {
        if self.timestamp_index_last >= 0 {
            let timestamp_index_last = self.timestamp_index_last as usize;
            if timestamp_index == timestamp_index_last {
                panic!(
                    "TODO: Timestamp index duplicated! {} == {}",
                    timestamp_index, timestamp_index_last
                );
            }
            // Add another block if the change insertion fails
            if !self.insert_change(timestamp_index - timestamp_index_last) {
                self.insert_block(timestamp_index, value_index);
            }
        } else {
            // Insert block since we have none currently
            self.insert_block(timestamp_index, value_index);
        }

        self.timestamp_index_last = timestamp_index as isize;
    }

    pub fn get_timestamp_index(&self, block_index: usize) -> usize {
        let range = (block_index * BLOCK_SIZE)..(block_index * BLOCK_SIZE + 8);
        u64::from_be_bytes((&self.blocks[range]).try_into().unwrap()) as usize
    }

    pub fn get_value_index(&self, block_index: usize) -> usize {
        let range = (block_index * BLOCK_SIZE + 8)..(block_index * BLOCK_SIZE + 16);
        u64::from_be_bytes((&self.blocks[range]).try_into().unwrap()) as usize
    }

    pub fn get_block_count(&self) -> usize {
        self.blocks.len() / BLOCK_SIZE
    }

    pub fn get_block_size(&self) -> usize {
        self.blocks.len()
    }

    fn search_timestamp_index_recursive(
        &self,
        timestamp_index: usize,
        block_range: Range<usize>,
    ) -> Option<WaveformSignalPosition> {
        if block_range.len() == 0 {
            return None;
        }
        // The start timestamp index is stored in eight bytes at the start of
        // this block
        let start_timestamp_index = self.get_timestamp_index(block_range.start);
        if timestamp_index < start_timestamp_index {
            // The timestamp index we are looking for exists before this range
            return None;
        }
        // Further narrow down the blocks to look at based on timestamp
        if block_range.len() > 1 {
            // Timestamp index was in the middle of the range, split it up
            let block_mid = (block_range.start + block_range.end) / 2;
            let mid_timestamp_index = self.get_timestamp_index(block_mid);
            return self.search_timestamp_index_recursive(
                timestamp_index,
                if timestamp_index < mid_timestamp_index {
                    block_range.start..block_mid
                } else {
                    block_mid..block_range.end
                },
            );
        }
        // We are only looking at one block now, iterate through it
        let mut pos = WaveformSignalPosition {
            index: WaveformSignalIndex {
                timestamp_index: 0,
                value_index: 0,
            },
            block_index: block_range.start,
            block_offset: 0,
            consumed_changes: 0,
        };
        // If the block was all skips then nothing is in it
        if !pos.consume_initial(&self) {
            return None;
        }
        pos.consumed_changes = 1;
        if pos.index.get_timestamp_index() > timestamp_index {
            // The current index is ahead of the search index
            return None;
        } else if pos.index.get_timestamp_index() == timestamp_index {
            // The current index is at the search index
            return Some(pos);
        }

        // Search through current block for timestamp index, returning the
        // prior one if we go past it
        Some(pos.block_seek(timestamp_index, &self))
    }

    /// Returns the waveform position for the change immediately before or at
    /// the requested timestamp index
    pub fn search_timestamp_index(&self, timestamp_index: usize) -> Option<WaveformSignalPosition> {
        self.search_timestamp_index_recursive(timestamp_index, 0..self.get_block_count())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaveformSignalIndex {
    timestamp_index: usize,
    value_index: usize,
}

impl WaveformSignalIndex {
    fn from_history(history: &WaveformSignalHistory, block_index: usize) -> Self {
        Self {
            timestamp_index: history.get_timestamp_index(block_index),
            value_index: history.get_value_index(block_index),
        }
    }

    pub fn get_timestamp_index(&self) -> usize {
        self.timestamp_index
    }

    pub fn get_value_index(&self) -> usize {
        self.value_index
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaveformSignalPosition {
    index: WaveformSignalIndex,
    block_index: usize,
    block_offset: usize,
    consumed_changes: u8,
}

impl WaveformSignalPosition {
    fn new() -> Self {
        Self {
            index: WaveformSignalIndex {
                timestamp_index: 0,
                value_index: 0,
            },
            block_index: 0,
            block_offset: 0,
            consumed_changes: 0,
        }
    }

    fn get_byte_index(&self) -> usize {
        self.block_index * BLOCK_SIZE + self.block_offset
    }

    fn consume_skips(&mut self, history: &WaveformSignalHistory) -> usize {
        let mut total_skips = 0usize;
        let mut skip_word = 0usize;
        let mut i = 0;
        while (self.block_offset < BLOCK_SIZE)
            && (history.blocks[self.get_byte_index()] & 128 != 128)
        {
            skip_word <<= 7;
            skip_word |= (history.blocks[self.get_byte_index()] & 127) as usize;
            self.block_offset += 1;
            i += 1;
            // Skips can be at most 8-bytes wide, then a new skip-word starts
            if i >= 8 {
                total_skips += skip_word;
                skip_word = 0;
            }
        }
        total_skips += skip_word;
        total_skips
    }

    fn consume_changes(&mut self, history: &WaveformSignalHistory) -> bool {
        // Byte indicates some number of changes - 1
        let available_changes = (history.blocks[self.get_byte_index()] & 127) + 1;
        if self.consumed_changes >= available_changes {
            self.consumed_changes = 0;
            self.block_offset += 1;
            false
        } else {
            self.consumed_changes += 1;
            self.index.timestamp_index += 1;
            self.index.value_index += 1;
            true
        }
    }

    fn consume_initial(&mut self, history: &WaveformSignalHistory) -> bool {
        // At the beginning of a block, read the starting indices
        self.index = WaveformSignalIndex::from_history(history, self.block_index);
        self.block_offset = 16;
        self.consumed_changes = 0;
        // Consume skips at the start
        self.index.timestamp_index += self.consume_skips(history);
        // If skips were the entire block then return false, look at next block
        self.block_offset < BLOCK_SIZE
    }

    /// Finds the next timestamp index where this signal changed
    pub fn next(&self, history: &WaveformSignalHistory) -> Option<Self> {
        let mut next = self.clone();
        loop {
            if next.block_offset == 0 {
                if next.consume_initial(history) {
                    next.consumed_changes = 1;
                    return Some(next);
                }
                continue;
            } else if next.block_offset >= BLOCK_SIZE {
                // At the end of a block, move onto the next one
                next.block_index += 1;
                next.block_offset = 0;
                next.consumed_changes = 0;
                if next.block_index >= history.get_block_count() {
                    return None;
                }
                continue;
            }
            // Consume skip(s)
            next.index.timestamp_index += next.consume_skips(history);
            // If we got to the end of the block then look at the next one
            if next.block_offset >= BLOCK_SIZE {
                continue;
            }
            // Consume a change if available, otherwise loop for next byte
            if next.consume_changes(history) {
                return Some(next);
            }
        }
    }

    /// Seeks the position through the current block of changes to immediately
    /// before or at the given timestamp index
    pub fn block_seek(&self, timestamp_index: usize, history: &WaveformSignalHistory) -> Self {
        let mut next = self.clone();
        loop {
            // Consume skip(s)
            let skips = next.consume_skips(history);
            if next.index.timestamp_index + skips > timestamp_index {
                return next;
            }
            next.index.timestamp_index += skips;
            // If we got to the end of the block then look at the next one
            if next.block_offset >= BLOCK_SIZE {
                return next;
            }
            // Determine if the change we are looking for is in this change block
            let requested_changes = timestamp_index - next.index.timestamp_index;
            let available_changes = (history.blocks[self.get_byte_index()] & 127) as usize + 1;
            //  - next.consumed_changes + 1;
            if available_changes - next.consumed_changes as usize > requested_changes {
                next.consumed_changes += requested_changes as u8;
                next.index.timestamp_index += requested_changes;
                next.index.value_index += requested_changes;
                return next;
            } else {
                next.block_offset += 1;
                next.consumed_changes = 0;
                next.index.timestamp_index += available_changes as usize - 1;
                next.index.value_index += available_changes as usize - 1;
            }
        }
    }

    pub fn get_index(&self) -> WaveformSignalIndex {
        self.index.clone()
    }
}

pub struct WaveformSignalHistoryIter<'a> {
    history: &'a WaveformSignalHistory,
    position: WaveformSignalPosition,
}

impl<'a> Iterator for WaveformSignalHistoryIter<'a> {
    type Item = WaveformSignalPosition;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(position) = self.position.next(&self.history) {
            self.position = position.clone();
            Some(position)
        } else {
            None
        }
    }
}

impl<'a> IntoIterator for &'a WaveformSignalHistory {
    type Item = WaveformSignalPosition;

    type IntoIter = WaveformSignalHistoryIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        WaveformSignalHistoryIter {
            history: self,
            position: WaveformSignalPosition::new(),
        }
    }
}

impl WaveformSignalHistory {
    pub fn iter_from_position(
        &self,
        position: WaveformSignalPosition,
    ) -> WaveformSignalHistoryIter {
        WaveformSignalHistoryIter {
            history: self,
            position: position,
        }
    }
}
