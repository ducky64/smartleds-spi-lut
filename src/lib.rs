#![cfg_attr(not(test), no_std)]

mod lut;
use lut::{SmartLedsSpiData};
pub use lut::{SmartLedsSpiBit, SmartLedsSpiLut};
mod bits_traits;
use bits_traits::{NumBits, TruncatedFrom};

use core::ops::{BitOr, Shl, Shr};

use embedded_hal_async::spi::SpiBus;

use smart_leds_trait::{SmartLedsWriteAsync, RGB8};


/// Write the colors to SPI bits in a buffer for transmission, returning the number of words.
fn write_buffer<Word, Lut, T, I>(lut: &Lut, iterator: T, buffer: &mut [Word]) -> usize 
where
    Lut: SmartLedsSpiData,
    Lut::Output: Copy + Default + BitOr<Lut::Output, Output = Lut::Output> + Shr<u8, Output = Lut::Output> + Shl<u8, Output = Lut::Output> + NumBits,
    Word: Copy + Default + TruncatedFrom<Lut::Output> + NumBits + 'static,
    T: IntoIterator<Item = I>,
    I: Into<RGB8>
{
    const {
        assert!(8 % Lut::INDEX_BITS == 0, "Lut::INDEX_BITS must divide from 8 (RGB8 component)");
        assert!(Lut::INDEX_BITS <= 8, "Lut::INDEX_BITS must be <= 8 (RGB8 component)");
        assert!(Lut::Output::BITS >= Word::BITS, "Lut::Output must larger or equal to than Word");
    };
    
    let output_to_word_shift = Lut::Output::BITS - Word::BITS;

    let mut buffer_index = 0;  // of the current word being written
    let mut accumulator: Lut::Output = Lut::Output::default();  // current SPI data being built, MSbit aligned
    let mut accumulator_bits = 0;  // number of bits written into accumulator

    for item in iterator {
        let item = item.into();
        for mut color_byte in [item.g, item.r, item.b] {
            for _ in 0..(8 / Lut::INDEX_BITS) {
                let lut_index = color_byte >> (8 - Lut::INDEX_BITS);
                let (spi_data, spi_bits) = lut.get(lut_index);
                color_byte = color_byte << Lut::INDEX_BITS;  // shift to get the next bits to the MSbits

                accumulator = accumulator | (spi_data >> accumulator_bits);
                let (new_accumulator_bits, spi_leftover_data, spi_leftover_bits) = if (spi_bits + accumulator_bits) >= Lut::Output::BITS {
                    (Lut::Output::BITS, spi_data << (Lut::Output::BITS - accumulator_bits), accumulator_bits + spi_bits - Lut::Output::BITS)
                } else {
                    (spi_bits + accumulator_bits, Lut::Output::default(), 0)
                };
                accumulator_bits = new_accumulator_bits;
                while accumulator_bits >= Word::BITS {
                    buffer[buffer_index] = Word::truncated_from(accumulator >> output_to_word_shift);
                    buffer_index += 1;
                    accumulator = accumulator << Word::BITS;
                    accumulator_bits -= Word::BITS;
                }

                accumulator = accumulator | (spi_leftover_data >> accumulator_bits);
                accumulator_bits += spi_leftover_bits;
            }
        }
    }

    while accumulator_bits >= Word::BITS {
        buffer[buffer_index] = Word::truncated_from(accumulator >> output_to_word_shift);
        buffer_index += 1;
        accumulator = accumulator << Word::BITS;
        accumulator_bits -= Word::BITS;
    }

    // shift any leftover bits in the last word
    if accumulator_bits > 0 {
        buffer[buffer_index] = Word::truncated_from(accumulator >> output_to_word_shift);
        buffer_index += 1;
    }

    buffer_index
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
    Lut::Output: Copy + Default + BitOr<Lut::Output, Output = Lut::Output> + Shr<u8, Output = Lut::Output> + Shl<u8, Output = Lut::Output> + NumBits,
    Word: Copy + Default + TruncatedFrom<Lut::Output> + NumBits + 'static,
{
    /// Creates a new instance given a SPI bus and lookup table defining how smartled bits are encoded into SPI bits.
    /// This requires the SPI driver to continuously transmit data, without inter-word gaps
    pub fn new(spi: SPI, lut: Lut) -> Self {
        Self { spi, lut, buffer: [[[Word::default(); WORDS_PER_COLOR]; 3]; N]}
    }

    fn flat_buffer(buffer: &mut [[[Word; WORDS_PER_COLOR]; 3]; N]) -> &mut [Word] {
        buffer.as_flattened_mut().as_flattened_mut()
    }
}

impl <Word, Lut, SPI, const N: usize, const WORDS_PER_COLOR: usize> SmartLedsWriteAsync
for SmartLedsSpi<Word, Lut, SPI, N, WORDS_PER_COLOR> 
where
    SPI: SpiBus<Word>,
    Lut: SmartLedsSpiData,
    Lut::Output: Copy + Default + BitOr<Lut::Output, Output = Lut::Output> + Shr<u8, Output = Lut::Output> + Shl<u8, Output = Lut::Output> + NumBits,
    Word: Copy + Default + TruncatedFrom<Lut::Output> + NumBits + 'static,
{
    type Error = SPI::Error;
    type Color = RGB8;

    async fn write<T, I>(&mut self, iterator: T) -> Result<(), Self::Error>
    where
        T: IntoIterator<Item = I>,
        I: Into<Self::Color>
    {
        let buffer = Self::flat_buffer(&mut self.buffer);
        let buffer_size = write_buffer(&self.lut, iterator, buffer);
        self.spi.write(&buffer[0..buffer_size]).await
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_zeros_2bit() {
        let lut = SmartLedsSpiLut::<u16, 4>::new(0b100, 3, 0b1100, 4);
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
}
