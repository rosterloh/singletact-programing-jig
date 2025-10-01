use crate::LED_STRING_SIZE;
use esp_hal::{
    Async,
    gpio::interconnect::PeripheralOutput,
    rmt::{ConstChannelAccess, Rmt, Tx},
};
use esp_hal_smartled::{SmartLedsAdapterAsync, buffer_size_async};
use smart_leds::{RGB8, SmartLedsWriteAsync};

/// We must know what the LED TX buffer size is as a constant for the types involved here
const LED_INTERNAL_BUF_LEN: usize = buffer_size_async(LED_STRING_SIZE);

/// Convenience type so we speak the same language when dealing with animations etc.
pub type LedBuffer = [RGB8; LED_STRING_SIZE];

/// Holds the state needed to drive the LED strip
pub struct LedDriver {
    /// Driver for the led array. We have to size it here to exactly what we will get back from
    /// the `SmartLedsAdapterAsync::new()` function when we set up the driver below
    led: SmartLedsAdapterAsync<ConstChannelAccess<Tx, 0>, LED_INTERNAL_BUF_LEN>,
}

impl LedDriver {
    /// Create a new driver for the LED string.
    ///
    /// # Parameters
    /// * `rmt` - The RMT peripheral device to use for driving the LED strip
    /// * `pin` - The GPIO pin to which the LED strip is connected
    pub fn new<'a>(rmt: Rmt<Async>, pin: impl PeripheralOutput<'a>) -> Self {
        //
        let channel = rmt.channel0;
        let buffer = [0_u32; buffer_size_async(LED_STRING_SIZE)];
        let led = SmartLedsAdapterAsync::new(channel, pin, buffer);
        Self { led }
    }
}

impl LedDriver {
    /// Update the contents of the buffer to the LED string, applying gamma correction and brightness.
    ///
    /// This must be called every time you want to propagate changes you have made to the string to
    /// the actual LED devices. This is not done automatically as you may want to do multiple changes
    /// before updating the display.
    ///
    /// # Parameters
    /// * `led_buffer` - Buffer containing LED values to write to the string
    /// * `brightness` - Global brightness level from 0 (off) to 255 (max brightness)
    pub async fn update_from_buffer(&mut self, led_buffer: &mut LedBuffer, brightness: u8) {
        let source = *led_buffer;
        let adjust_iter =
            smart_leds::brightness(smart_leds::gamma(source.iter().cloned()), brightness);
        for (pix, corrected) in led_buffer.iter_mut().zip(adjust_iter) {
            *pix = corrected;
        }
        self.led
            .write(*led_buffer)
            .await
            .expect("Failed to update LED driver");
    }

    /// Switches all the LEDS off
    #[allow(unused)]
    pub async fn all_off(&mut self) {
        self.update_from_buffer(&mut LedBuffer::default(), 0).await;
    }

    /// Switches all the LEDS to white at the specified brightness.
    ///
    /// # Parameters
    /// * `brightness` - The brightness level to set all LEDs to, from 0 (off) to 255 (full brightness)
    pub async fn white(&mut self, brightness: u8) {
        let mut b = LedBuffer::default();
        b.fill(RGB8 {
            r: 255,
            g: 255,
            b: 255,
        });
        self.update_from_buffer(&mut b, brightness).await;
    }
}
