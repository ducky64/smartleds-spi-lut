#![no_std]

mod lut;
use lut::{SmartLedsSpiData};
pub use lut::{SmartLedsSpiBit, SmartLedsSpiLut};
mod bits_traits;
use bits_traits::{NumBits, TruncatedFrom};


use core::ops::{BitOr, Shl, Shr};
use core::cmp::min;

use embedded_hal_async::spi::SpiBus;

use smart_leds_trait::{SmartLedsWriteAsync, RGB8};


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
    Lut::Output: Copy + Shr<u8, Output = Lut::Output> + NumBits,
    Word: Copy + Default + TruncatedFrom<Lut::Output> + Shl<u8, Output = Word> + BitOr<Word, Output = Word> + NumBits + 'static,
{
    /// Creates a new instance given a SPI bus and lookup table defining how smartled bits are encoded into SPI bits.
    /// This requires the SPI driver to continuously transmit data, without inter-word gaps
    pub fn new(spi: SPI, lut: Lut) -> Self {
        Self { spi, lut, buffer: [[[Word::default(); WORDS_PER_COLOR]; 3]; N]}
    }

    fn flat_buffer(buffer: &mut [[[Word; WORDS_PER_COLOR]; 3]; N]) -> &mut [Word] {
        buffer.as_flattened_mut().as_flattened_mut()
    }

    /// Write the colors to SPI bits in a buffer for transmission, returning the number of words.
    fn write_buffer<T, I>(lut: &Lut, iterator: T, buffer: &mut [Word]) -> usize 
    where
        T: IntoIterator<Item = I>,
        I: Into<RGB8>
    {
        const {
            assert!(8 % Lut::INDEX_BITS == 0, "Lut::INDEX_BITS must divide from 8 (RGB8 component)");
            assert!(Lut::INDEX_BITS <= 8, "Lut::INDEX_BITS must be <= 8 (RGB8 component)");
            assert!(Lut::Output::BITS >= Word::BITS, "Lut::Output must larger or equal to than Word");
        };
        
        let index_mask = (1 << Lut::INDEX_BITS) - 1;

        let mut buffer_index = 0;  // of the current word being written
        let mut word_buffer: Word = Word::default();  // accumulates SPI bits per word, LSbit aligned
        let mut word_bits = 0;  // number of bits written into word_buffer

        for item in iterator {
            let item = item.into();
            for color_byte in [item.g, item.r, item.b] {
                let mut written_bits: u8 = 0;
                while written_bits < 8 {
                  let lut_index = (color_byte >> (8 - written_bits - Lut::INDEX_BITS)) & index_mask;
                  let (spi_data, mut spi_bits) = lut.get(lut_index);
                  written_bits += Lut::INDEX_BITS;

                  // spi_data is shifted into word_buffer MSbit first
                  // spi_bits tracks the most significant valid bit in spi_data
                  while spi_bits > 0 {
                      let shift_bits = min(spi_bits, Word::BITS - word_bits);
                      if shift_bits < Word::BITS {
                          word_buffer = (word_buffer << shift_bits) | Word::truncated_from(spi_data >> (spi_bits - shift_bits));
                      } else {  // a full shift panics
                          word_buffer = Word::truncated_from(spi_data >> (spi_bits - shift_bits));
                      }
                      
                      word_bits += shift_bits;
                      spi_bits -= shift_bits;

                      if word_bits >= Word::BITS {
                          buffer[buffer_index] = word_buffer;
                          buffer_index += 1;
                          word_buffer = Word::default();
                          word_bits = 0;
                      }
                  }
                }
            }
        }

        // shift any leftover bits in the last word
        if word_bits > 0 {
            buffer[buffer_index] = word_buffer << (Word::BITS - word_bits);
            buffer_index += 1;
        }

        buffer_index
    }
}

impl <Word, Lut, SPI, const N: usize, const WORDS_PER_COLOR: usize> SmartLedsWriteAsync
for SmartLedsSpi<Word, Lut, SPI, N, WORDS_PER_COLOR> 
where
    SPI: SpiBus<Word>,
    Lut: SmartLedsSpiData,
    Lut::Output: Copy + Shr<u8, Output = Lut::Output> + NumBits,
    Word: Copy + Default + TruncatedFrom<Lut::Output> + Shl<u8, Output = Word> + BitOr<Word, Output = Word> + NumBits + 'static,
{
    type Error = SPI::Error;
    type Color = RGB8;

    async fn write<T, I>(&mut self, iterator: T) -> Result<(), Self::Error>
    where
        T: IntoIterator<Item = I>,
        I: Into<Self::Color>
    {
        let buffer = Self::flat_buffer(&mut self.buffer);
        let buffer_size = Self::write_buffer(&self.lut, iterator, buffer);
        self.spi.write(&buffer[0..buffer_size]).await
    }
}
