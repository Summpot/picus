// Copyright 2019 the Xilem Authors and the Druid Authors
// SPDX-License-Identifier: Apache-2.0

use std::cmp::Ordering;

/// An `f32` value which can move towards a target value at a linear rate over time.
#[derive(Clone, Debug)]
pub struct AnimatedF32 {
    /// The value which self will eventually reach.
    target: f32,
    /// The current value.
    value: f32,
    // TODO: Provide different easing functions, instead of just linear.
    /// The change in value every millisecond, which will not change over the lifetime of the value.
    rate_per_millisecond: f32,
}

impl AnimatedF32 {
    /// Creates a value which is not changing.
    pub fn stable(value: f32) -> Self {
        assert!(value.is_finite(), "invalid animated value");
        Self {
            target: value,
            value,
            rate_per_millisecond: 0.,
        }
    }

    /// Moves this value to the `target` over `over_millis` milliseconds.
    /// Might change the current value, if `over_millis` is zero.
    ///
    /// `over_millis` should be non-negative.
    ///
    /// # Panics
    ///
    /// If `target` is not a finite value.
    pub fn move_to(&mut self, target: f32, over_millis: f32) {
        assert!(target.is_finite(), "invalid target value");
        assert!(over_millis.is_finite(), "invalid delay value");
        self.target = target;
        match over_millis.partial_cmp(&0.) {
            Some(Ordering::Equal) => self.value = target,
            Some(Ordering::Less) => {
                tracing::warn!("move_to: provided negative time step {over_millis}");
                self.value = target;
            }
            Some(Ordering::Greater) => {
                self.rate_per_millisecond = (self.target - self.value) / over_millis;
                debug_assert!(
                    self.rate_per_millisecond.is_finite(),
                    "Calculated invalid rate despite valid inputs. Current value is {}",
                    self.value
                );
            }
            None => panic!("Provided invalid time step {over_millis}"),
        }
    }

    /// Advances this animation by `by_millis` milliseconds.
    ///
    /// Returns the status of the animation after this advancement.
    pub fn advance(&mut self, by_millis: f32) -> AnimationStatus {
        assert!(by_millis.is_finite(), "invalid timestep value");

        let original_side = self
            .value
            .partial_cmp(&self.target)
            .expect("Target and value are not NaN.");

        self.value += self.rate_per_millisecond * by_millis;
        let other_side = self
            .value
            .partial_cmp(&self.target)
            .expect("Target and value are not NaN.");

        if other_side.is_eq() || original_side != other_side {
            self.value = self.target;
            self.rate_per_millisecond = 0.;
            AnimationStatus::Completed
        } else {
            AnimationStatus::Ongoing
        }
    }

    /// Returns the current value.
    pub fn value(&self) -> f32 {
        self.value
    }
}

/// The status an animation can be in.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AnimationStatus {
    /// The animation has finished.
    Completed,
    /// The animation is still running.
    Ongoing,
}

impl AnimationStatus {
    /// Return true if animation has finished.
    pub fn is_completed(self) -> bool {
        matches!(self, Self::Completed)
    }
}
