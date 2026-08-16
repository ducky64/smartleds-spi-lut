use core::ops::{BitOr, Shl};
use crate::bits_traits::NumBits;


pub trait SmartLedsSpiData {
    const INDEX_BITS: u8;  // number of bits this table indexes by
    type Output;  // return type

    /// Given BITS of smartled bits, return the corresponding SPI word and number of SPI bits
    /// Data must be MSbit aligned
    fn get(&self, value: u8) -> (Self::Output, u8);
}


/// Define a simple lookup table indexed one bit at a time.
/// zero: SPI data for a smartled zero bit, occupying the LSbits
/// zero_bits: number of SPI bits of the zero value
/// one, one_bits: same for a smartled one bit
/// zero_bits, one_bits do not need to divide cleanly into a Word
///   the SPI buffer generation will build Words across smartled bit boundaries
pub struct SmartLedsSpiBit {
    zero: u8,
    zero_bits: u8,
    one: u8,
    one_bits: u8
}

impl SmartLedsSpiBit {
    /// Creates a new one-bit lookup table, input data is LSbit aligned
    pub fn new(zero: u8, zero_bits: u8, one: u8, one_bits: u8) -> Self {
        Self { zero: zero << (8 - zero_bits), zero_bits, one: one << (8 - one_bits), one_bits }
    }
}

impl SmartLedsSpiData for SmartLedsSpiBit {
  const INDEX_BITS: u8 = 1;
  type Output = u8;

  fn get(&self, value: u8) -> (Self::Output, u8) {
      if value & 0x01 == 0 {
          (self.zero, self.zero_bits)
      } else {
          (self.one, self.one_bits)
      }
  }
}


pub struct SmartLedsSpiLut<T, const N: usize>
{
    table: [T; N],
    bits: [u8; N],
}

impl <T, const N: usize> SmartLedsSpiLut<T, N>
where T: Copy + Default + Shl<u8, Output = T> + BitOr<T, Output = T> + NumBits
{
    const INDEX_MASK : u8 = (N as u8) - 1;

    pub fn new(zero: T, zero_bits: u8, one: T, one_bits: u8) -> Self {
        let mut table: [T; N] = [T::default(); N];
        let mut bits: [u8; N] = [0; N];

        for entry in 0..N {
            let mut value: T = T::default();
            let mut bit_count: u8 = 0;
            for bit in (0..Self::INDEX_BITS).rev() {
                if entry & (1 << bit) == 0 {
                    value = (value << zero_bits) | zero;
                    bit_count += zero_bits;
                } else {
                    value = (value << one_bits) | one;
                    bit_count += one_bits;
                }
            }
            table[entry] = value << (T::BITS - bit_count);
            bits[entry] = bit_count;
        }

        Self { table, bits }
    }
}

impl <T, const N: usize> SmartLedsSpiData for SmartLedsSpiLut<T, N> 
where T: Copy + Default + Shl<u8, Output = T> + BitOr<T, Output = T> + NumBits
{
  const INDEX_BITS: u8 = N.ilog2() as u8;
  type Output = T;

  fn get(&self, value: u8) -> (Self::Output, u8) {
      (self.table[(value & Self::INDEX_MASK) as usize], self.bits[(value & Self::INDEX_MASK) as usize])
  }
}
