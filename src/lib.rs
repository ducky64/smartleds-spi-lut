#![cfg_attr(not(test), no_std)]

mod lut;
use lut::{SmartLedsSpiData};
pub use lut::{SmartLedsSpiBit, SmartLedsSpiLut};
mod bits_traits;
use bits_traits::{NumBits, WordOps};

use embedded_hal_async::spi::SpiBus;

use smart_leds_trait::{SmartLedsWriteAsync, RGB8};


/// Write the colors to SPI bits in a buffer for transmission, returning the number of words.
fn write_buffer<Word, Lut, T, I>(lut: &Lut, iterator: T, buffer: &mut [Word]) -> usize 
where
    Lut: SmartLedsSpiData,
    Word: Copy + Default + WordOps + NumBits + 'static,
    T: IntoIterator<Item = I>,
    I: Into<RGB8>
{
    const {
        assert!(8 % Lut::INDEX_BITS == 0, "Lut::INDEX_BITS must divide from 8 (RGB8 component)");
        assert!(Lut::INDEX_BITS <= 8, "Lut::INDEX_BITS must be <= 8 (RGB8 component)");
    };
    
    let accumulator_to_word_shift = 32 - Word::BITS;

    let start_ptr = buffer.as_mut_ptr();
    let end_ptr = unsafe { start_ptr.add(buffer.len()) };
    let mut ptr = start_ptr;

    let mut push_word = |b: Word| {
        unsafe {
            assert!(ptr < end_ptr, "buffer overflow");
            ptr.write(b);
            ptr = ptr.add(1);
        }
    };

    let mut accumulator: u32 = 0;  // current SPI data being built, MSbit aligned
    let mut accumulator_bits: usize = 0;  // number of bits written into accumulator

    for item in iterator {
        let item = item.into();
        let mut color_data = (item.g as u32) << 16 | (item.r as u32) << 8 | (item.b as u32);

        for _ in 0..(24 / Lut::INDEX_BITS) {
            let lut_index = color_data >> (24 - Lut::INDEX_BITS);
            let (spi_data, spi_bits) = lut.get(lut_index);
            color_data = color_data << Lut::INDEX_BITS;  // shift to get the next bits to the MSbits

            accumulator |= spi_data >> accumulator_bits;
            accumulator_bits += spi_bits as usize;
            while accumulator_bits >= Word::BITS as usize {
                push_word(Word::truncate_from_u32(accumulator >> accumulator_to_word_shift));
                if Word::BITS < 32 {  // note, shift panics if shifting out whole word
                    accumulator <<= Word::BITS;
                } else {
                    accumulator = 0;
                }
                accumulator_bits -= Word::BITS as usize;
            }
        }
    }

    // shift any leftover bits in the last word
    if accumulator_bits > 0 {
        push_word(Word::truncate_from_u32(accumulator >> accumulator_to_word_shift));
    }

    unsafe { ptr.offset_from(start_ptr) as usize }
}

/// WS2812 LED driver with customizable SPI word size and bit encoding
/// N is the maximum number of LEDs in the chain, for buffer sizing
/// WORDS_PER_COLOR is the maximum number of SPI words used to encode a single color of LED data
///   This encoding is needed for internal array sizing without generic const exprs
pub struct SmartLedsSpi<Word, Lut, SPI, const N: usize, const WORDS_PER_COLOR: usize> {
    spi: SPI,
    lut: Lut,
    buffer: [[[Word; WORDS_PER_COLOR]; 3]; N],
}

impl <Word: Copy + 'static, Lut, SPI, const N: usize, const WORDS_PER_COLOR: usize> SmartLedsSpi<Word, Lut, SPI, N, WORDS_PER_COLOR> 
where
    SPI: SpiBus<Word>,
    Lut: SmartLedsSpiData,
    Word: Copy + Default + WordOps + NumBits + 'static,
{
    /// Creates a new instance given a SPI bus and lookup table defining how smartled bits are encoded into SPI bits.
    /// This requires the SPI driver to continuously transmit data, without inter-word gaps
    pub fn new(spi: SPI, lut: Lut) -> Self {
        Self { spi, lut, buffer: [[[Word::default(); WORDS_PER_COLOR]; 3]; N]}
    }
}

impl <Word, Lut, SPI, const N: usize, const WORDS_PER_COLOR: usize> SmartLedsWriteAsync
for SmartLedsSpi<Word, Lut, SPI, N, WORDS_PER_COLOR> 
where
    SPI: SpiBus<Word>,
    Lut: SmartLedsSpiData,
    Word: Copy + Default + WordOps + NumBits + 'static,
{
    type Error = SPI::Error;
    type Color = RGB8;

    async fn write<T, I>(&mut self, iterator: T) -> Result<(), Self::Error>
    where
        T: IntoIterator<Item = I>,
        I: Into<Self::Color>
    {
        let buffer = self.buffer.as_flattened_mut().as_flattened_mut();
        let buffer_size = write_buffer(&self.lut, iterator, buffer);
        self.spi.write(&buffer[0..buffer_size]).await
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_zeros_2bit() {
        let lut = SmartLedsSpiLut::<u8, 4>::new(0b100, 3, 0b1100, 4);
        let rgb = [RGB8 { r: 0b00_00_00_00, g: 0b00_00_00_00, b: 0b00_00_00_00 }];
        let mut buffer = [0u8; 12];
        let words = write_buffer(&lut, rgb, &mut buffer);

        assert_eq!(words, 9);

        assert_eq!(buffer[0], 0b100_100_10);
        assert_eq!(buffer[1], 0b0_100_100_1);
        assert_eq!(buffer[2], 0b00_100_100);

        assert_eq!(buffer[3], 0b100_100_10);
        assert_eq!(buffer[4], 0b0_100_100_1);
        assert_eq!(buffer[5], 0b00_100_100);

        assert_eq!(buffer[6], 0b100_100_10);
        assert_eq!(buffer[7], 0b0_100_100_1);
        assert_eq!(buffer[8], 0b00_100_100);
    }

    #[test]
    fn test_buffer_zeros_4bit() {
        let lut = SmartLedsSpiLut::<u16,16>::new(0b100, 3, 0b1100, 4);
        let rgb = [RGB8 { r: 0b00_00_00_00, g: 0b00_00_00_00, b: 0b00_00_00_00 }];
        let mut buffer = [0u8; 12];
        let words = write_buffer(&lut, rgb, &mut buffer);

        assert_eq!(words, 9);

        assert_eq!(buffer[0], 0b100_100_10);
        assert_eq!(buffer[1], 0b0_100_100_1);
        assert_eq!(buffer[2], 0b00_100_100);

        assert_eq!(buffer[3], 0b100_100_10);
        assert_eq!(buffer[4], 0b0_100_100_1);
        assert_eq!(buffer[5], 0b00_100_100);

        assert_eq!(buffer[6], 0b100_100_10);
        assert_eq!(buffer[7], 0b0_100_100_1);
        assert_eq!(buffer[8], 0b00_100_100);
    }

    #[test]
    fn test_buffer_1bit() {
        let lut = SmartLedsSpiBit::new(0b100, 3, 0b1100, 4);
        let rgb = [RGB8 { r: 0b00_00_00_00, g: 0b00_00_00_00, b: 0b00_00_00_10 }];
        let mut buffer = [0u8; 12];
        let words = write_buffer(&lut, rgb, &mut buffer);

        assert_eq!(words, 10);

        assert_eq!(buffer[0], 0b100_100_10);
        assert_eq!(buffer[1], 0b0_100_100_1);
        assert_eq!(buffer[2], 0b00_100_100);

        assert_eq!(buffer[3], 0b100_100_10);
        assert_eq!(buffer[4], 0b0_100_100_1);
        assert_eq!(buffer[5], 0b00_100_100);

        assert_eq!(buffer[6], 0b100_100_10);
        assert_eq!(buffer[7], 0b0_100_100_1);
        assert_eq!(buffer[8], 0b00_1100_10);
        assert_eq!(buffer[9], 0b0_0000000);
    }

    #[test]
    fn test_buffer_mixed_4bit() {
        let lut = SmartLedsSpiLut::<u16,16>::new(0b100, 3, 0b1100, 4);
        let rgb = [RGB8 { r: 0b00_00_00_00, g: 0b00_00_00_00, b: 0b00_10_10_11 }];
        let mut buffer = [0u8; 12];
        let words = write_buffer(&lut, rgb, &mut buffer);

        assert_eq!(words, 10);

        assert_eq!(buffer[0], 0b100_100_10);
        assert_eq!(buffer[1], 0b0_100_100_1);
        assert_eq!(buffer[2], 0b00_100_100);

        assert_eq!(buffer[3], 0b100_100_10);
        assert_eq!(buffer[4], 0b0_100_100_1);
        assert_eq!(buffer[5], 0b00_100_100);

        assert_eq!(buffer[6], 0b100_100_11);
        assert_eq!(buffer[7], 0b00_100_110);
        assert_eq!(buffer[8], 0b0_100_1100);
        assert_eq!(buffer[9], 0b1100_0000);  // LSbit padded
    }

    #[test]
    #[should_panic]
    fn test_buffer_overflow_4bit() {
        let lut = SmartLedsSpiLut::<u16,16>::new(0b100, 3, 0b1100, 4);
        let rgb = [RGB8 { r: 0b00_00_00_00, g: 0b00_00_00_00, b: 0b00_10_10_11 }];
        let mut buffer = [0u8; 9];
        write_buffer(&lut, rgb, &mut buffer);
    }
}
