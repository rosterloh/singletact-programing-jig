#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
// use alloc::{boxed::Box, rc::Rc};
use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_futures::yield_now;
use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyleBuilder, iso_8859_9::FONT_10X20},
    pixelcolor::BinaryColor,
    prelude::Point,
    text::{Baseline, Text},
};
// use esp_backtrace as _;
use esp_hal::{
    Config,
    clock::CpuClock,
    i2c::master::{Config as I2cConfig, I2c},
    rmt::Rmt,
    time::Rate,
    timer::timg::TimerGroup,
};
use panic_rtt_target as _;
use singletact_programing_jig::{
    BUTTON_STATE, ButtonEvent,
    tasks::{handle_button, handle_neopixel},
};
use ssd1306::{
    I2CDisplayInterface, Ssd1306Async, mode::DisplayConfigAsync, prelude::DisplayRotation,
    size::DisplaySize128x64,
};

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    #[cfg(all(feature = "rtt", feature = "defmt"))]
    rtt_target::rtt_init_defmt!();

    // esp_alloc::heap_allocator!(size: (48 + 96) * 1024);

    let peripherals = esp_hal::init(Config::default().with_cpu_clock(CpuClock::max()));
    let timer_group_0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timer_group_0.timer0);

    spawner
        .spawn(handle_button(peripherals.GPIO3, peripherals.GPIO9))
        .unwrap();

    let frequency = Rate::from_mhz(80);
    let rmt = Rmt::new(peripherals.RMT, frequency)
        .expect("Failed to initialise RMT0")
        .into_async();
    spawner
        .spawn(handle_neopixel(
            rmt.channel0,
            peripherals.GPIO2,
            peripherals.RNG,
        ))
        .unwrap();

    let i2c = I2c::new(peripherals.I2C0, I2cConfig::default())
        .unwrap()
        .with_scl(peripherals.GPIO6)
        .with_sda(peripherals.GPIO5)
        .into_async();
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306Async::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    if let Err(_e) = display.init().await {
        // error!("Error: {:?}", e);
        error!("Display couldn't be initialised");
        loop {
            yield_now().await;
        }
    }
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_10X20)
        .text_color(BinaryColor::On)
        .build();

    display.clear_buffer();
    Text::with_baseline(
        "Press button\nto start",
        Point::zero(),
        text_style,
        Baseline::Top,
    )
    .draw(&mut display)
    .unwrap();
    display.flush().await.unwrap();

    loop {
        match BUTTON_STATE.wait().await {
            ButtonEvent::Press => {
                info!("Button Pressed");
            }
            ButtonEvent::HoldHalfSecond => {
                info!("Button Held Half Second");
            }
            ButtonEvent::HoldFullSecond => {
                info!("Button Held Full Second");
            }
        }
    }
}
