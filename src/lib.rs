#![no_std]

pub mod animations;
pub mod drivers;
pub mod tasks;

pub use tasks::*;

/// The display animation update interval in milliseconds
pub const ANIMATION_UPDATE: u64 = 250;

/// The default colour for the LED strip (green)
pub const DEFAULT_COLOUR: [u8; 3] = [0, 255, 0];

/// The number of LEDs in the string we are driving
pub const LED_STRING_SIZE: usize = 1;

/// The maximum number of pending animations in the animation queue
pub const MAX_PENDING_ANIMATIONS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RgbBrightness {
    Low = 10,
    Medium = 100,
    High = 200,
    Max = 255,
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
