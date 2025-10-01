use crate::animations::{Animation, SparkleAnimation, is_interruptable, next_buffer};
use crate::{
    ANIMATION_UPDATE, DEFAULT_COLOUR, MAX_PENDING_ANIMATIONS,
    drivers::neopixel::{LedBuffer, LedDriver},
};
use defmt::{debug, error, info};
use embassy_futures::{
    select::{Either, select},
    yield_now,
};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};
use embassy_time::{Duration, Ticker};
use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyleBuilder, iso_8859_9::FONT_10X20},
    pixelcolor::BinaryColor,
    prelude::Point,
    text::{Baseline, Text},
};
use esp_hal::{Async, i2c::master::I2c};
use heapless::spsc::Queue;
use smart_leds::RGB8;
use ssd1306::{
    I2CDisplayInterface, Ssd1306Async, mode::DisplayConfigAsync, prelude::DisplayRotation,
    size::DisplaySize128x64,
};

/// Manage the display state by sending it messages of this type. If anyone asks why I like Rust,
/// this is one of the many reasons
#[allow(unused)]
pub enum DisplayState {
    /// Suspends animation update
    Stop,
    /// Restart animation update
    Start,
    /// Switch of all the LEDs, stopping animation
    Off,
    /// Start the animation again
    On,
    /// Show initial message and wait for button press
    Init,
    /// Enable/disable torch function
    Torch(bool),
    /// Set the display brightness
    Brightness(u8),
    /// Set the address of the sensor at the given position
    SetAddress(u8),
}

const DISPLAY_QUEUE_SIZE: usize = 10;
/// Channel types for the display task.
pub type DisplayChannel = Channel<CriticalSectionRawMutex, DisplayState, DISPLAY_QUEUE_SIZE>;
pub type DisplayChannelSender =
    Sender<'static, CriticalSectionRawMutex, DisplayState, DISPLAY_QUEUE_SIZE>;
pub type DisplayChannelReceiver =
    Receiver<'static, CriticalSectionRawMutex, DisplayState, DISPLAY_QUEUE_SIZE>;

/// Display driver main task.
/// The display is fully managed from this task. It contains the state and responds to messages
/// sent to it via the channel.
///
/// # Parameters
/// * `channel` - Channel receiver for display state messages
/// * `led` - LED driver instance for controlling the LED strip
/// * `default` - Default animation type to use when no other animation is queued. T
///
#[embassy_executor::task]
pub async fn display_task(
    channel: &'static DisplayChannelReceiver,
    led: &'static mut LedDriver,
    i2c: &'static mut I2c<'static, Async>,
) {
    let mut animation = Ticker::every(Duration::from_millis(ANIMATION_UPDATE));
    let mut running = true;
    let mut animation_queue: Queue<Animation, MAX_PENDING_ANIMATIONS> = Queue::new();
    let mut current_animation = Animation::Sparkle(SparkleAnimation::new(
        RGB8::from(DEFAULT_COLOUR),
        Some(Duration::from_secs(2)),
    ));
    let mut brightness: u8 = 10;
    let mut torch = false;

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
        Baseline::Middle,
    )
    .draw(&mut display)
    .unwrap();
    display.flush().await.unwrap();

    info!("DISPLAY_TASK: Task started. Waiting for messages...");
    loop {
        // Wait for one of our futures to become ready
        match select(animation.next(), channel.receive()).await {
            // Animation update timer
            Either::First(_) => {
                // The ticker woke us up
                if running {
                    // Look at our state and return something that we can display.
                    // Note we must peek into animation_queue because if we are interruptable, we must
                    // leave the next animation in the queue until the current animation terminates.
                    let mut new_buf: Option<LedBuffer> = match (
                        next_buffer(&mut current_animation),
                        animation_queue.peek(),
                        is_interruptable(&current_animation),
                    ) {
                        // A new animation and the current one is interruptable, set up the new one.
                        (_, Some(animation), true) => {
                            debug!(
                                "DISPLAY_TASK: Animation {} replaced by updated {}",
                                current_animation, animation
                            );
                            current_animation = animation.clone();
                            animation_queue.dequeue().unwrap(); // Infallible drop because the peek was Some()
                            next_buffer(&mut current_animation)
                        }
                        // Just one animation running, so let it roll
                        (Some(buf), None, _) => {
                            debug!(
                                "DISPLAY_TASK: Animation continuing with {}",
                                current_animation
                            );
                            Some(buf)
                        }
                        // A new animation available but we are not interruptable, return the current animation next buffer
                        (Some(buf), Some(animation), false) => {
                            debug!(
                                "DISPLAY_TASK: Uninterruptible animation {} updated with pending animation {}",
                                current_animation, animation
                            );
                            Some(buf)
                        }
                        // Current animation terminates, no new animation so revert to default
                        (None, None, _) => {
                            debug!("DISPLAY_TASK: No animations found. Reverting to the default");
                            // current_animation = default.clone();
                            led.all_off().await;
                            next_buffer(&mut current_animation)
                        }
                        // No new buffer and a pending animation
                        (None, Some(animation), _) => {
                            debug!(
                                "DISPLAY_TASK: No current animation with a pending animation {}",
                                animation
                            );
                            current_animation = animation.clone();
                            animation_queue.dequeue().unwrap(); // Infallible drop because the peek was Some()
                            next_buffer(&mut current_animation)
                        }
                    };
                    // The buffer is still wrapped in an option, so grab it. It will never be None
                    if let Some(ref mut b) = new_buf {
                        led.update_from_buffer(b, brightness).await;
                    } // Just let the default animation pick this one up if we don't have a new buffer
                }
            }
            // Control message from our channel
            Either::Second(message) => {
                // We received a message
                use DisplayState::*;
                match message {
                    Stop => running = false,
                    Start => running = true,
                    Off => {
                        led.all_off().await;
                        running = false;
                    }
                    On => {
                        running = true;
                    }
                    Init => {
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
                    }
                    Brightness(b) => {
                        brightness = b;
                        if torch {
                            led.white(brightness).await;
                        }
                    }
                    Torch(on) => {
                        if on {
                            running = false;
                            torch = true;
                            led.white(brightness).await;
                        } else {
                            running = true;
                            torch = false;
                            led.all_off().await;
                        };
                    }
                    SetAddress(pos) => {
                        display.clear_buffer();
                        let addr = pos + 0x08;
                        let mut msg = heapless::String::<32>::new();
                        ufmt::uwrite!(msg, "Position: {}\nAddress: 0x{:x}", pos, addr).unwrap();
                        Text::with_baseline(msg.as_str(), Point::zero(), text_style, Baseline::Top)
                            .draw(&mut display)
                            .unwrap();
                        display.flush().await.unwrap();
                    }
                }
            }
        };
    }
}
