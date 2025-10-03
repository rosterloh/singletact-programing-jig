#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

// use alloc::{boxed::Box, rc::Rc};
use defmt::info;
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Timer};
use esp_hal::{
    Async,
    Config,
    clock::CpuClock,
    gpio::{Input, InputConfig, Pull},
    i2c::master::{Config as I2cConfig, I2c},
    rmt::Rmt,
    // rng::Rng,
    time::Rate,
    timer::{systimer::SystemTimer /*timg::TimerGroup,*/},
};
use panic_rtt_target as _;
use singletact_programing_jig::{
    drivers::{button::wait_for_press, neopixel::LedDriver},
    tasks::display::{
        DisplayChannel, DisplayChannelReceiver, /*DisplayChannelSender, */ DisplayState,
        display_task,
    },
};

use static_cell::StaticCell;

/// Communicate with the display task using this channel and the DisplayState enum
// static DISPLAY_SENDER: StaticCell<DisplayChannelSender> = StaticCell::new();
static DISPLAY_RECEIVER: StaticCell<DisplayChannelReceiver> = StaticCell::new();
static DISPLAY_CHANNEL: StaticCell<DisplayChannel> = StaticCell::new();

/// Our LED driver that underlies the display task
static LED_DRIVER: StaticCell<LedDriver> = StaticCell::new();

/// I2c bus shared between display and sensors
static I2C_BUS: StaticCell<I2cBus> = StaticCell::new(); // I2c<'static, Async>

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    #[cfg(all(feature = "rtt", feature = "defmt"))]
    rtt_target::rtt_init_defmt!();

    // esp_alloc::heap_allocator!(size: (48 + 96) * 1024);

    let peripherals = esp_hal::init(Config::default().with_cpu_clock(CpuClock::max()));
    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    // let timer1 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timer0.alarm0);

    let display_channel = DISPLAY_CHANNEL.init(Channel::new());
    let sender = display_channel.sender();
    // let ble_sender = DISPLAY_SENDER.init(sender);
    let receiver = DISPLAY_RECEIVER.init(display_channel.receiver());
    // let mut rng = Rng::new(peripherals.RNG);

    let rmt = Rmt::new(peripherals.RMT, Rate::from_mhz(80))
        .expect("Failed to initialise RMT0")
        .into_async();
    let led_driver = LED_DRIVER.init(LedDriver::new(rmt, peripherals.GPIO2));
    let i2c = I2C_BUS.init(I2cBus::new(
        I2c::new(peripherals.I2C0, I2cConfig::default())
            .unwrap()
            .with_scl(peripherals.GPIO6)
            .with_sda(peripherals.GPIO5)
            .into_async(),
    ));
    // Start the display manager task
    spawner
        .spawn(display_task(receiver, led_driver, i2c))
        .expect("Failed to spawn display task");

    // Set up buttons for the functions we need
    let config = InputConfig::default().with_pull(Pull::Up);
    let mut button0 = Input::new(peripherals.GPIO9, config);
    let mut button1 = Input::new(peripherals.GPIO3, config);

    info!("MAIN: Starting main loop");
    sender.send(DisplayState::Init).await;
    let mut torch = false;
    loop {
        match select(wait_for_press(&mut button0), wait_for_press(&mut button1)).await {
            Either::First(_) => {
                info!("MAIN: Toggling torch mode {}", torch);
                torch ^= true;
                sender.send(DisplayState::Torch(torch)).await;
            }
            Either::Second(_) => {
                info!("MAIN: Starting device programming");
                for i in 0..8 {
                    sender.send(DisplayState::SetAddress(i)).await;
                    Timer::after(Duration::from_secs(1)).await;
                }
                sender.send(DisplayState::Init).await;
            }
        };
        info!("MAIN: Button pressed");
    }
}
