pub trait UnsignedInteger {
    fn from_usize(u: usize) -> Self;
    fn to_usize(&self) -> usize;
    fn get_width(&self) -> u32;
}

impl UnsignedInteger for u8 {
    fn from_usize(u: usize) -> Self {
        u as u8
    }
    fn to_usize(&self) -> usize {
        *self as usize
    }
    fn get_width(&self) -> u32 {
        u8::BITS
    }
}

impl UnsignedInteger for u16 {
    fn from_usize(u: usize) -> Self {
        u as u16
    }
    fn to_usize(&self) -> usize {
        *self as usize
    }
    fn get_width(&self) -> u32 {
        u16::BITS
    }
}

impl UnsignedInteger for u32 {
    fn from_usize(u: usize) -> Self {
        u as u32
    }
    fn to_usize(&self) -> usize {
        *self as usize
    }
    fn get_width(&self) -> u32 {
        u32::BITS
    }
}

impl UnsignedInteger for u64 {
    fn from_usize(u: usize) -> Self {
        u as u64
    }
    fn to_usize(&self) -> usize {
        *self as usize
    }
    fn get_width(&self) -> u32 {
        u64::BITS
    }
}

impl UnsignedInteger for usize {
    fn from_usize(u: usize) -> Self {
        u
    }
    fn to_usize(&self) -> usize {
        *self as usize
    }
    fn get_width(&self) -> u32 {
        usize::BITS
    }
}
