#[test]
fn test_bitvector() {
    use crate::bitvector::*;

    fn check_two_state(bv: &mut BitVector) {
        for i in 0..bv.get_bit_width() {
            bv.set_bit(i, Logic::One);
            assert_eq!(bv.get_bit(i), Logic::One);
            bv.set_bit(i, Logic::Zero);
            assert_eq!(bv.get_bit(i), Logic::Zero);
        }

        let mut prng: u32 = 0xdeadbeef;
        let mut v = Vec::new();
        for i in 0..bv.get_bit_width() {
            let bit = Logic::from(prng & 1 == 1);
            bv.set_bit(i, bit);
            v.push(bit);
            prng ^= (prng << 13) ^ (prng >> 17) ^ (prng << 5);
        }

        let bvc = bv.clone();
        for i in 0..bv.get_bit_width() {
            assert_eq!(bv.get_bit(i), v[i]);
            assert_eq!(bvc.get_bit(i), v[i]);
        }
    }

    fn check_four_state(bv: &mut BitVector) {
        check_two_state(bv);
        for i in 0..bv.get_bit_width() {
            bv.set_bit(i, Logic::One);
            assert_eq!(bv.get_bit(i), Logic::One);
            bv.set_bit(i, Logic::Zero);
            assert_eq!(bv.get_bit(i), Logic::Zero);
            for b in [
                Logic::Zero,
                Logic::One,
                Logic::Unknown,
                Logic::HighImpedance,
            ] {
                bv.set_bit(i, b);
                assert_eq!(bv.get_bit(i), b);
            }
        }

        let mut prng: u32 = 0xdeadbeef;
        let mut v = Vec::new();
        for i in 0..bv.get_bit_width() {
            let bit = Logic::from((prng & 1 == 1, prng & 2 == 1));
            bv.set_bit(i, bit);
            v.push(bit);
            prng ^= (prng << 13) ^ (prng >> 17) ^ (prng << 5);
        }

        let bvc = bv.clone();
        for i in 0..bv.get_bit_width() {
            assert_eq!(bv.get_bit(i), v[i]);
            assert_eq!(bvc.get_bit(i), v[i]);
        }
    }

    // Check behavior of non-pointer bit vectors
    let bv = BitVector::new(0, false);
    assert_eq!(bv.get_bit_width(), 0);
    assert!(!bv.is_four_state());
    assert!(!bv.is_pointer());
    assert_eq!(bv.get_memory_words_size(), 0);

    let bv = BitVector::new(1, false);
    assert_eq!(bv.get_bit_width(), 1);
    assert!(!bv.is_four_state());
    assert!(!bv.is_pointer());

    let bv = BitVector::new(0, true);
    assert_eq!(bv.get_bit_width(), 0);
    assert!(bv.is_four_state());
    assert!(!bv.is_pointer());

    let bv = BitVector::new(1, true);
    assert_eq!(bv.get_bit_width(), 1);
    assert!(bv.is_four_state());
    assert!(!bv.is_pointer());

    let mut bv = BitVector::new(64, false);
    assert_eq!(bv.get_bit_width(), 64);
    assert!(!bv.is_pointer());
    check_two_state(&mut bv);

    let mut bv = BitVector::new(32, true);
    assert_eq!(bv.get_bit_width(), 32);
    assert!(!bv.is_pointer());
    check_four_state(&mut bv);

    // Check behavior of pointer bit vectors
    let mut bv = BitVector::new(65, false);
    assert_eq!(bv.get_bit_width(), 65);
    assert_eq!(bv.get_memory_words_size(), 2);
    assert_eq!(bv.get_vector_words_size(), 2);
    assert!(bv.is_pointer());
    check_two_state(&mut bv);

    let mut bv = BitVector::new(33, true);
    assert_eq!(bv.get_bit_width(), 33);
    assert_eq!(bv.get_memory_words_size(), 2);
    assert_eq!(bv.get_vector_words_size(), 1);
    assert!(bv.is_pointer());
    check_four_state(&mut bv);

    for i in 7..16 {
        let bit_width = 1usize << i;

        let mut bv = BitVector::new(bit_width, false);
        assert_eq!(bv.get_bit_width(), bit_width);
        assert_eq!(bv.get_memory_words_size(), bit_width / 64);
        assert_eq!(bv.get_vector_words_size(), bit_width / 64);
        assert!(bv.is_pointer());
        check_two_state(&mut bv);
        let mut bv = bv.clone();
        check_two_state(&mut bv);

        let mut bv = BitVector::new(bit_width, true);
        assert_eq!(bv.get_bit_width(), bit_width);
        assert_eq!(bv.get_memory_words_size(), bit_width / 64 * 2);
        assert_eq!(bv.get_vector_words_size(), bit_width / 64);
        assert!(bv.is_pointer());
        check_four_state(&mut bv);
        let mut bv = bv.clone();
        check_four_state(&mut bv);
    }

    let bv = BitVector::from_ascii(b"1001");
    assert_eq!(bv.get_bit(0), Logic::One);
    assert_eq!(bv.get_bit(1), Logic::Zero);
    assert_eq!(bv.get_bit(2), Logic::Zero);
    assert_eq!(bv.get_bit(3), Logic::One);

    let bv = BitVector::from_ascii_four_state(b"zx01");
    assert_eq!(bv.get_bit(0), Logic::One);
    assert_eq!(bv.get_bit(1), Logic::Zero);
    assert_eq!(bv.get_bit(2), Logic::Unknown);
    assert_eq!(bv.get_bit(3), Logic::HighImpedance);

    let bv =
        BitVector::from_ascii(b"01010010110100101101001011010010110100101101001011010010110100101");
    assert_eq!(bv.get_bit_width(), 65);
    assert_eq!(bv.get_bit(0), Logic::One);
    assert_eq!(bv.get_bit(1), Logic::Zero);
    assert_eq!(bv.get_bit(63), Logic::One);
    assert_eq!(bv.get_bit(64), Logic::Zero);

    let bv = BitVector::from_ascii_four_state(
        b"0X0100101101001011010010110100101101001011010010110100101101001Z1",
    );
    assert_eq!(bv.get_bit_width(), 65);
    assert_eq!(bv.get_bit(0), Logic::One);
    assert_eq!(bv.get_bit(1), Logic::HighImpedance);
    assert_eq!(bv.get_bit(63), Logic::Unknown);
    assert_eq!(bv.get_bit(64), Logic::Zero);

    let value = &[0xAA];
    let bv = BitVector::from_be_bytes_two_state(8, value);
    assert_eq!(bv.get_bit(0), Logic::Zero);
    assert_eq!(bv.get_bit(7), Logic::One);
    assert!(!bv.is_pointer());
    let value_test = &mut [0u8; 1];
    bv.to_be_bytes_two_state(&mut value_test[..]);
    assert_eq!(value, value_test);
    assert_eq!(bv.to_bits_two_state::<u8>(), 0xAA);
    assert_eq!(bv.to_bits_two_state::<u16>(), 0xAA);
    assert_eq!(bv.to_bits_two_state::<u32>(), 0xAA);
    assert_eq!(bv.to_bits_two_state::<u64>(), 0xAA);
    assert_eq!(bv.to_bits_two_state::<usize>(), 0xAA);
    assert_eq!(bv.to_bits_four_state::<u8>(), (0xAA, 0));

    let value = &[0xAA];
    let mask = &[0x81];
    let bv = BitVector::from_be_bytes_four_state(8, value, mask);
    assert!(!bv.is_pointer());
    let value_test = &mut [0u8; 1];
    let mask_test = &mut [0u8; 1];
    bv.to_be_bytes_four_state(&mut value_test[..], &mut mask_test[..]);
    assert_eq!(value, value_test);
    assert_eq!(mask, mask_test);
    assert_eq!(bv.to_bits_four_state::<u8>(), (0xAA, 0x81));
    assert_eq!(bv.to_bits_four_state::<u16>(), (0xAA, 0x81));
    assert_eq!(bv.to_bits_four_state::<u32>(), (0xAA, 0x81));

    let value = &[0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01];
    let bv = BitVector::from_be_bytes_two_state(64, value);
    assert_eq!(bv.get_bit(0), Logic::One);
    assert_eq!(bv.get_bit(1), Logic::Zero);
    assert_eq!(bv.get_bit(63), Logic::One);
    assert!(!bv.is_pointer());
    let value_test = &mut [0u8; 8];
    bv.to_be_bytes_two_state(&mut value_test[..]);
    assert_eq!(value, value_test);

    let value = &[0x1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01];
    let bv = BitVector::from_be_bytes_two_state(65, value);
    assert_eq!(bv.get_bit(0), Logic::One);
    assert_eq!(bv.get_bit(1), Logic::Zero);
    assert_eq!(bv.get_bit(64), Logic::One);
    assert!(bv.is_pointer());
    let value_test = &mut [0u8; 9];
    bv.to_be_bytes_two_state(&mut value_test[..]);
    assert_eq!(value, value_test);

    let value = &[0x1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01];
    let mask = &[0x1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let bv = BitVector::from_be_bytes_four_state(65, value, mask);
    assert_eq!(bv.get_bit(0), Logic::One);
    assert_eq!(bv.get_bit(1), Logic::Zero);
    assert_eq!(bv.get_bit(64), Logic::HighImpedance);
    assert!(bv.is_pointer());
    let value_test = &mut [0u8; 9];
    let mask_test = &mut [0u8; 9];
    bv.to_be_bytes_four_state(&mut value_test[..], &mut mask_test[..]);
    assert_eq!(value, value_test);
    assert_eq!(mask, mask_test);

    // Check that strings get printed correctly
    assert_eq!(
        BitVector::from_ascii(b"1001").to_string_radix(BitVectorRadix::Binary),
        "b1001"
    );
    assert_eq!(
        BitVector::from_ascii_four_state(b"Z00X").to_string_radix(BitVectorRadix::Binary),
        "bZ00X"
    );
    assert_eq!(
        BitVector::from_ascii(b"1010").to_string_radix(BitVectorRadix::Hexadecimal),
        "hA"
    );
    assert_eq!(
        BitVector::from_ascii(b"11010").to_string_radix(BitVectorRadix::Hexadecimal),
        "h1A"
    );
    assert_eq!(
        BitVector::from_ascii_four_state(b"1101X").to_string_radix(BitVectorRadix::Hexadecimal),
        "h1X"
    );
    assert_eq!(
        BitVector::from_ascii_four_state(b"1101X").to_string_radix(BitVectorRadix::Hexadecimal),
        "h1X"
    );
    assert_eq!(
        BitVector::from_ascii_four_state(b"1101Z").to_string_radix(BitVectorRadix::Hexadecimal),
        "h1Z"
    );
    assert_eq!(
        BitVector::from_ascii_four_state(b"110ZX").to_string_radix(BitVectorRadix::Hexadecimal),
        "h1X"
    );
    assert_eq!(
        BitVector::from_ascii(b"11011").to_string_radix(BitVectorRadix::Decimal),
        "d27"
    );
    assert_eq!(
        BitVector::from_ascii_four_state(b"11011").to_string_radix(BitVectorRadix::Decimal),
        "d27"
    );
    assert_eq!(
        BitVector::from_ascii_four_state(b"110ZX").to_string_radix(BitVectorRadix::Decimal),
        "dX"
    );
    assert_eq!(
        BitVector::from_ascii_four_state(b"110ZZ").to_string_radix(BitVectorRadix::Decimal),
        "dZ"
    );
}
