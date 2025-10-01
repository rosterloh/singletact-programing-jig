use core::pin;

// use embassy_futures::select::{Either, select};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Instant, Timer};
use esp_hal::{
    gpio::{self, InputConfig, OutputConfig, Pull},
    peripherals::{GPIO3, GPIO9},
};
use futures::future::{Either, select};
use smart_leds::RGB8;

use crate::{RGB_CONFIG, RgbMode};

pub static BUTTON_STATE: Signal<CriticalSectionRawMutex, ButtonEvent> = Signal::new();

#[derive(Debug, Clone, PartialEq)]
pub enum ButtonEvent {
    Press,
    HoldHalfSecond,
    HoldFullSecond,
}

#[embassy_executor::task]
pub async fn handle_button(led_pin: GPIO3<'static>, button_pin: GPIO9<'static>) {
    let mut led = gpio::Output::new(led_pin, gpio::Level::Low, OutputConfig::default());
    let mut button = gpio::Input::new(button_pin, InputConfig::default().with_pull(Pull::Up));
    loop {
        button.wait_for_low().await;
        let time_down = Instant::now();
        led.set_high();
        let wait_for_high = pin::pin!(button.wait_for_high());
        let res = select(wait_for_high, Timer::after_millis(500)).await;
        match res {
            Either::Left((_value1, _future2)) => {}
            Either::Right((_value2, button_release)) => {
                // In this case, the button is being held, so set colour and wait for release

                let previous_mode: RgbMode;
                {
                    let mut config = RGB_CONFIG.lock().await;
                    previous_mode = config.rgb_mode.clone();
                    // "White"
                    config.set_mode(RgbMode::Static(RGB8::new(190, 240, 255)));
                }
                match select(button_release, Timer::after_millis(500)).await {
                    // Button released before next 0.5s
                    Either::Left(_) => {}
                    Either::Right((_, button_release)) => {
                        {
                            RGB_CONFIG
                                .lock()
                                .await
                                .set_mode(RgbMode::Static(RGB8::new(0, 0, 255)))
                        }
                        button_release.await;
                    }
                }
                RGB_CONFIG.lock().await.set_mode(previous_mode)
            }
        }

        let duration_pressed = Instant::now() - time_down;
        led.set_low();
        let button_event = if duration_pressed > Duration::from_ticks(25000) {
            if duration_pressed > Duration::from_millis(1000) {
                ButtonEvent::HoldFullSecond
            } else if duration_pressed > Duration::from_millis(500) {
                ButtonEvent::HoldHalfSecond
            } else {
                ButtonEvent::Press
            }
        } else {
            continue;
        };
        defmt::dbg!("Button Press: ", &button_event);
        BUTTON_STATE.signal(button_event);
    }
}
