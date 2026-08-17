# smartleds-spi-lut
smart-leds-trait compatible driver using the SPI perpheral and optimized buffer generation with custom bit patterns and bit lengths

This generates a `const` lookup table (LUT) mapping smartled bits to SPI words, allowing it to efficiently turn smartled colors into SPI buffer data.

LUT entries count must be a power of 2 and the index bitcount must evenly divided in a smartled color channel (u8).

The LUT can be constructed with custom bit patterns to support different SPI peripherals and clocks.
Bit patterns can be of arbitrary length, the example has different lengths for the zero and one pattern and is compliant with datasheet timing requirements.

## Example

```rust
use smart_leds::{RGB8, SmartLedsWriteAsync};
use smartleds_spi_lut::{SmartLedsSpi, SmartLedsSpiLut};

// platform-specific SPI init here, this is for CH32V003
let mut spi_config = hal::spi::Config::default();
spi_config.frequency = Hertz::khz(3000);  // device limited to fractions of main clock
spi_config.mode = embedded_hal::spi::MODE_1;  // avoids extra leading edge on SPI
let spi = Spi::new_txonly_nosck::<0>(p.SPI1, p.PC6, p.DMA1_CH3, spi_config);

// 16 entry lut = 4 bits at a time
let mut colors = [RGB8 { r: 0, g: 0, b: 0 }; 11];
let ws_lut = SmartLedsSpiLut::<u16, 16>::new(0b100, 3, 0b1100, 4);
// 11 colors, u8 SPI word, maximum 4 SPI bytes per color channel (4 SPI bits per smartled bit)
let mut ws = SmartLedsSpi::<u8, _, _, 11, 4>::new(spi, ws_lut);

ws.write(colors.into_iter()).await.ok();
```
