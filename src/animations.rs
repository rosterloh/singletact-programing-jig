//! Animations module provides different LED animation patterns.
//!
//! This module contains implementations for various LED animations including:
//! - Sparkle animations that create random brightness variations of a single colour
//! - Presence animations that display and rotate colours representing visible souls

use crate::drivers::neopixel::LedBuffer;
use defmt::{Format, Formatter, write};
use embassy_time::{Duration, Instant};
use smart_leds::RGB8;

/// Represents different types of animations that can be displayed on the LED strip
#[derive(Clone)]
pub enum Animation {
    /// Animation that creates a sparkling effect with random brightness variations
    Sparkle(SparkleAnimation),
    // /// Animation that oscillates brightness to create a breathing effect
    // Breathe(BreatheAnimation),
}

/// Checks if the given animation can be interrupted
///
/// # Arguments
/// * `anim` - Reference to the Animation to check
///
/// # Returns
/// True if the animation can be interrupted, false otherwise
pub fn is_interruptable(anim: &Animation) -> bool {
    match anim {
        Animation::Sparkle(s) => s.is_interruptable(),
        // Animation::Breathe(s) => s.is_interruptable(),
    }
}

/// Helper function to get the new buffer regardless of animation. This is because we cannot use
///  [dyn traits](https://doc.rust-lang.org/rust-by-example/trait/dyn.html) in a `no_std` without
/// setting up a heap.
///
/// # Arguments
/// * `anim` - A mutable reference to the Animation enum that will generate the next buffer state
/// # Returns
/// The result of the iterator on the animation
pub fn next_buffer(anim: &mut Animation) -> Option<LedBuffer> {
    match anim {
        Animation::Sparkle(s) => s.next(),
        // Animation::Breathe(s) => s.next(),
    }
}

impl Format for Animation {
    fn format(&self, fmt: Formatter) {
        match self {
            Animation::Sparkle(_) => write!(fmt, "Sparkle"),
            // Animation::Breathe(_) => write!(fmt, "Breathe"),
        }
    }
}

pub trait Interruptable {
    /// If this is true then the animation is interruptable before its iterator returns None
    /// If a new soul arrives, we want it to sparkle for a few seconds and not be interrupted
    /// by a new arrival. Those can sit in the queue until this one is done. Be careful here
    /// as this could block all future animations sitting in the queue.
    fn is_interruptable(&self) -> bool;
}

/// Takes one colour and generates a random brightness up to the maximum brightness
/// specified. It will continue to return `Some(buffer)` until the expiry time is reached
/// if one was specified
#[derive(Clone)]
pub struct SparkleAnimation {
    /// The colour to sparkle
    colour: RGB8,
    /// The system time at which the animation should expire. If it is None, the animation
    /// will run but will mark itself as interruptable.
    expires: Option<Instant>,
    /// Random number generator for the sparkle effect
    rng: fastrand::Rng,
}

impl Iterator for SparkleAnimation {
    type Item = LedBuffer;

    fn next(&mut self) -> Option<Self::Item> {
        let done = match self.expires {
            Some(exp) if Instant::now() < exp => false, // Have expiration but not expired so not done
            None => false,                              // No expiration is never done
            _ => true,                                  // All other cases are done
        };

        if !done {
            let mut buffer = LedBuffer::default();
            for led in buffer.iter_mut() {
                let b = self.rng.u8(0..255);
                *led = set_brightness(b, self.colour);
            }
            Some(buffer)
        } else {
            None
        }
    }
}

impl Interruptable for SparkleAnimation {
    fn is_interruptable(&self) -> bool {
        self.expires.is_none()
    }
}

impl SparkleAnimation {
    /// Creates a new SparkleAnimation instance that generates random brightness variations of a base colour
    ///
    /// # Arguments
    /// * `colour` - The base RGB colour to be used for the sparkle effect
    /// * `ttl` - Optional Duration that specifies how long the animation should run. None implies indefinitely
    ///
    /// Returns a new SparkleAnimation instance initialised with the current time as the RNG seed and
    /// the specified parameters. The animation will be interruptible if no ttl is provided
    pub(crate) fn new(colour: RGB8, ttl: Option<Duration>) -> Self {
        let seed = Instant::now().as_ticks();
        let expires = ttl.map(|t| Instant::now() + t);
        Self {
            colour,
            expires,
            rng: fastrand::Rng::with_seed(seed),
        }
    }
}

// #[derive(Clone)]
// pub enum Direction {
//     Up,
//     Down,
// }

// #[derive(Clone)]
// pub struct BreatheAnimation {
//     brightness: u8,
//     direction: Direction,
//     step: i16,
//     min: u8,
// }

// impl BreatheAnimation {
//     /// Create a BreatheAnimation.
//     ///
//     /// # Parameters
//     /// * `brightness` - Initial brightness value (0-255)
//     /// * `direction` - Initial direction of brightness change (Up or Down)
//     /// * `step` - Amount to change brightness by in each iteration
//     /// * `min` - Minimum brightness value to not go below
//     #[allow(unused)]
//     pub(crate) fn new(brightness: u8, direction: Direction, step: u8, min: u8) -> Self {
//         Self {
//             brightness,
//             direction,
//             step: step as i16,
//             min,
//         }
//     }

//     /// Create a throbber starting at a random brightness and vary it with a random step in a
//     /// random direction.
//     ///
//     /// # Parameters
//     /// * `min` - Minimum brightness value to not go below
//     #[allow(unused)]
//     pub fn new_random(min: u8) -> Self {
//         let seed = Instant::now().as_ticks();
//         let mut rng = fastrand::Rng::with_seed(seed);
//         Self {
//             brightness: rng.u8(min..),
//             direction: if rng.bool() {
//                 Direction::Up
//             } else {
//                 Direction::Down
//             },
//             step: rng.i16(8..64),
//             min,
//         }
//     }
// }

// impl Iterator for BreatheAnimation {
//     type Item = u8;

//     /// Next brightness value for this breathe animation
//     fn next(&mut self) -> Option<Self::Item> {
//         match self.direction {
//             Direction::Up => {
//                 self.brightness = clip(self.brightness as i16 + self.step);
//                 if self.brightness == 255 {
//                     self.direction = Direction::Down;
//                 }
//             }
//             Direction::Down => {
//                 self.brightness = clip_min(self.brightness as i16 - self.step, self.min);
//                 if self.brightness == self.min {
//                     self.direction = Direction::Up;
//                 }
//             }
//         };
//         Some(self.brightness)
//     }
// }

pub fn set_brightness(brightness: u8, pixel: RGB8) -> RGB8 {
    if brightness == 0 {
        return RGB8::default();
    }
    if brightness == 255 {
        return pixel;
    }
    // Use u16 for the multiplication to avoid overflow before the division.
    let r = ((pixel.r as u16 * brightness as u16) / 255) as u8;
    let g = ((pixel.g as u16 * brightness as u16) / 255) as u8;
    let b = ((pixel.b as u16 * brightness as u16) / 255) as u8;

    RGB8::new(r, g, b)
}

pub fn clip(v: i16) -> u8 {
    if v < 0 {
        0
    } else if v > 255 {
        255
    } else {
        v as u8
    }
}

/// Clip to a minimum value
pub fn clip_min(v: i16, min: u8) -> u8 {
    if v < min as i16 { min } else { v as u8 }
}
