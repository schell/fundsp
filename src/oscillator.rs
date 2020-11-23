use super::audionode::*;
use super::math::*;
use super::*;
use numeric_array::*;
use std::marker::PhantomData;

/// Sine oscillator.
#[derive(Clone)]
pub struct SineComponent<T: Float> {
    _marker: PhantomData<T>,
    phase: f64,
    sample_duration: f64,
    hash: u32,
}

impl<T: Float> SineComponent<T> {
    pub fn new() -> SineComponent<T> {
        SineComponent {
            _marker: PhantomData,
            phase: 0.0,
            sample_duration: 1.0 / DEFAULT_SR,
            hash: 0,
        }
    }
}

impl<T: Float> AudioNode for SineComponent<T> {
    const ID: u32 = 21;
    type Sample = T;
    type Inputs = typenum::U1;
    type Outputs = typenum::U1;

    fn reset(&mut self, sample_rate: Option<f64>) {
        self.phase = rnd(self.hash as u64);
        if let Some(sr) = sample_rate {
            self.sample_duration = 1.0 / sr
        };
    }

    #[inline]
    fn tick(
        &mut self,
        input: &Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let frequency = input[0].to_f64();
        self.phase += frequency * self.sample_duration;
        [convert(sin(self.phase * TAU))].into()
    }

    #[inline]
    fn set_hash(&mut self, hash: u32) {
        self.hash = hash;
    }
}
