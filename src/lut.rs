use crate::bits_traits::{WordOps, NumBits};


pub trait SmartLedsSpiData {
    const INDEX_BITS: usize;  // number of bits this table indexes by

    /// Given BITS of smartled bits, return the corresponding SPI word and number of SPI bits
    /// Data is MSbit aligned
    fn get(&self, value: u32) -> (u32, u8);
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
  const INDEX_BITS: usize = 1;

  fn get(&self, value: u32) -> (u32, u8) {
      if value & 0x01 == 0 {
          ((self.zero as u32) << 24, self.zero_bits)
      } else {
          ((self.one as u32) << 24, self.one_bits)
      }
  }
}


pub struct SmartLedsSpiLut<T, const N: usize>
{
    table: [T; N],
    bits: [u8; N],
}

impl <T, const N: usize> SmartLedsSpiLut<T, N>
{
    const INDEX_MASK : u32 = (N - 1) as u32;
}

macro_rules! impl_lut_table {
    ($($ty:ty),*) => { $(
        impl <const N: usize> SmartLedsSpiLut<$ty, N>
        {
            pub const fn new(zero: $ty, zero_bits: u8, one: $ty, one_bits: u8) -> Self {
                let mut table: [$ty; N] = [0; N];
                let mut bits: [u8; N] = [0; N];
                let mut entry: usize = 0;
                while entry < N {
                    let mut value: $ty = 0;
                    let mut bit_count: u8 = 0;
                    let mut bit = Self::INDEX_BITS;
                    while bit > 0 {
                        bit -= 1;
                        if entry & (1 << bit) == 0 {
                            value = (value << zero_bits) | zero;
                            bit_count += zero_bits;
                        } else {
                            value = (value << one_bits) | one;
                            bit_count += one_bits;
                        }
                    }
                    table[entry] = value << ((<$ty>::BITS as u8) - bit_count);
                    bits[entry] = bit_count;

                    entry += 1;
                }

                Self { table, bits }
            }
        }
    )* }
}

impl <T, const N: usize> SmartLedsSpiData for SmartLedsSpiLut<T, N> 
where T: Copy + WordOps + NumBits
{
    const INDEX_BITS: usize = N.ilog2() as usize;

    fn get(&self, value: u32) -> (u32, u8) {
        let index = (value & Self::INDEX_MASK) as usize;
        (self.table[index].to_u32() << (32 - T::BITS), self.bits[index])
  }
}

// only support u8 and u16, to allow for lossless shifting in u32 accumulator
impl_lut_table!(u8, u16);


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lut_2bit() {
        let lut = SmartLedsSpiLut::<u8, 4>::new(0b100, 3, 0b1100, 4);

        assert_eq!(lut.get(0b00), (0b100_100 << 26, 6));
        assert_eq!(lut.get(0b01), (0b100_1100 << 25, 7));
        assert_eq!(lut.get(0b10), (0b1100_100 << 25, 7));
        assert_eq!(lut.get(0b11), (0b1100_1100 << 24, 8));
    }

    #[test]
    fn test_compile_time_const_evaluation() {
        pub static CONST_LUT: SmartLedsSpiLut<u8, 4> =
            SmartLedsSpiLut::<u8, 4>::new(0b100, 3, 0b1100, 4);
            
        assert_eq!(CONST_LUT.get(0b10), (0b1100_100 << 25, 7));
    }

    #[test]
    fn test_lut_4bit() {
        let lut = SmartLedsSpiLut::<u16, 16>::new(0b100, 3, 0b1100, 4);

        assert_eq!(lut.get(0b0000), (0b100_100_100_100 << 20, 12));
        assert_eq!(lut.get(0b0110), (0b100_1100_1100_100 << 18, 14));
        assert_eq!(lut.get(0b1010), (0b1100_100_1100_100 << 18, 14));
        assert_eq!(lut.get(0b1111), (0b1100_1100_1100_1100 << 16, 16));
    }
}
