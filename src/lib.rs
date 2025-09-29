#![no_std]
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex, watch::Watch};
use smart_leds::RGB8;

pub mod maths;
pub mod tasks;
pub use tasks::button::{BUTTON_STATE, ButtonEvent};
pub use tasks::{handle_button, handle_neopixel};

// Idea for DEFAULT value in trait from https://docs.rs/const-default/latest/const_default/trait.ConstDefault.html
pub trait ConstDefault: Sized {
    const DEFAULT: Self;
}

pub static RGB_CONFIG: Mutex<CriticalSectionRawMutex, RgbConfig> = Mutex::new(RgbConfig::DEFAULT);
pub static RGB_CONFIG_UPDATED: Watch<CriticalSectionRawMutex, u8, 4> = Watch::new();

#[derive(Clone, Debug, PartialEq)]
pub enum RgbMode {
    SineCycle(f32),
    Continuous(u32),
    Random(u32),
    Fibonacci(u32),
    Static(RGB8),
}
impl ConstDefault for RgbMode {
    const DEFAULT: Self = Self::SineCycle(0.01);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RgbBrightness {
    Low = 10,
    Medium = 100,
    High = 200,
    Max = 255,
}
impl ConstDefault for RgbBrightness {
    const DEFAULT: Self = Self::Low;
}

/// Values roughly model an exponential curve (rounded to the nearest integer)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RgbRate {
    VerySlow = 1,
    Slow = 3,
    Moderate = 7,
    Fast = 20,
    VeryFast = 55,
}
impl ConstDefault for RgbRate {
    const DEFAULT: Self = Self::Moderate;
}

#[derive(Clone, Debug, PartialEq)]
pub struct RgbConfig {
    pub rgb_mode: RgbMode,
    pub rgb_brightness: RgbBrightness,
    pub rgb_rate_modifier: RgbRate,
}

impl RgbConfig {
    pub fn new(
        rgb_mode: RgbMode,
        rgb_brightness: RgbBrightness,
        rgb_rate_modifier: RgbRate,
    ) -> Self {
        Self {
            rgb_mode,
            rgb_brightness,
            rgb_rate_modifier,
        }
    }
    pub async fn from_environment() -> Self {
        RGB_CONFIG.lock().await.clone()
    }
    pub async fn apply(self) {
        *RGB_CONFIG.lock().await = self
    }
    pub fn set_mode(&mut self, rgb_mode: RgbMode) {
        self.rgb_mode = rgb_mode;
    }
    pub fn set_brightness(&mut self, rgb_brightness: RgbBrightness) {
        self.rgb_brightness = rgb_brightness;
    }
    pub fn set_rate(&mut self, rgb_rate_modifier: RgbRate) {
        self.rgb_rate_modifier = rgb_rate_modifier;
    }
}

impl ConstDefault for RgbConfig {
    const DEFAULT: Self = Self {
        rgb_mode: RgbMode::DEFAULT,
        rgb_brightness: RgbBrightness::DEFAULT,
        rgb_rate_modifier: RgbRate::DEFAULT,
    };
}
