use super::audionode::*;
use super::*;
use numeric_array::*;

/// Maximum length sequences (MLS) are pseudorandom, spectrally flat,
/// binary white noise sequences with interesting properties.
/// We have pre-baked sequences with state space sizes from 1 to 31 bits.
#[derive(Copy, Clone)]
pub struct Mls {
    /// State space size in bits.
    n: u32,
    /// Current state.
    s: u32,
}

// Feedback table for MLS sequence generation.
static MLS_POLY: [u32; 31] = [
    0b1,
    0b11,
    0b110,
    0b1100,
    0b10100,
    0b110000,
    0b1001000,
    0b10111000,
    0b100010000,
    0b1001000000,
    0b10100000000,
    0b110010100000,
    0b1101100000000,
    0b11000010001000,
    0b110000000000000,
    0b1101000000001000,
    0b10010000000000000,
    0b100000010000000000,
    0b1100011000000000000,
    0b10010000000000000000,
    0b101000000000000000000,
    0b1100000000000000000000,
    0b10000100000000000000000,
    0b111000010000000000000000,
    0b1001000000000000000000000,
    0b10000000000000000000100011,
    0b100000000000000000000010011,
    0b1001000000000000000000000000,
    0b10100000000000000000000000000,
    0b100000000000000000000000101001,
    0b1001000000000000000000000000000,
];

impl Mls {
    /// Creates a MLS.
    /// Number of bits in the state space is n (1 <= n <= 31).
    pub fn new(n: u32) -> Mls {
        assert!(n >= 1 && n <= 31);
        Mls { n, s: (1 << n) - 1 }
    }

    /// Creates a MLS from seed.
    /// Number of bits in the state space is n (1 <= n <= 31).
    pub fn new_with_seed(n: u32, seed: u32) -> Mls {
        assert!(n >= 1 && n <= 31);
        Mls {
            n,
            s: 1 + seed % ((1 << n) - 1),
        }
    }

    /// Sequence length. The sequence repeats after 2**n - 1 steps.
    pub fn length(self) -> u32 {
        (1 << self.n) - 1
    }

    /// Returns the next state in the sequence.
    pub fn next(self) -> Mls {
        let feedback = MLS_POLY[(self.n - 1) as usize] & self.s;
        let parity = feedback.count_ones() & 1;
        Mls {
            n: self.n,
            s: ((self.s << 1) | parity) & self.length(),
        }
    }

    /// The current value in the sequence, either 0 or 1.
    pub fn value(self) -> u32 {
        (self.s >> (self.n - 1)) & 1
    }
}

/// MLS noise component.
#[derive(Clone)]
pub struct MlsNoise<T> {
    _marker: std::marker::PhantomData<T>,
    mls: Mls,
    hash: u32,
}

impl<T: Float> MlsNoise<T> {
    pub fn new(mls: Mls) -> MlsNoise<T> {
        MlsNoise {
            _marker: std::marker::PhantomData,
            mls,
            hash: 0,
        }
    }
}

impl<T: Float> AudioNode for MlsNoise<T> {
    const ID: u32 = 19;
    type Sample = T;
    type Inputs = typenum::U0;
    type Outputs = typenum::U1;

    fn reset(&mut self, _sample_rate: Option<f64>) {
        self.mls = Mls::new_with_seed(self.mls.n, self.hash);
    }

    #[inline]
    fn tick(
        &mut self,
        _input: &Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let value = T::new(self.mls.value() as i64);
        self.mls = self.mls.next();
        [value * T::new(2) - T::new(1)].into()
    }

    #[inline]
    fn set_hash(&mut self, hash: u32) {
        self.hash = hash;
    }
}

/// White noise component.
#[derive(Clone)]
pub struct NoiseNode<T> {
    _marker: std::marker::PhantomData<T>,
    x: u64,
    hash: u32,
}

impl<T: Float> NoiseNode<T> {
    pub fn new() -> NoiseNode<T> {
        NoiseNode {
            _marker: std::marker::PhantomData,
            x: 0,
            hash: 0,
        }
    }
}

impl<T: Float> AudioNode for NoiseNode<T> {
    const ID: u32 = 20;
    type Sample = T;
    type Inputs = typenum::U0;
    type Outputs = typenum::U1;

    fn reset(&mut self, _sample_rate: Option<f64>) {
        self.x = self.hash as u64;
    }

    #[inline]
    fn tick(
        &mut self,
        _input: &Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        self.x = self
            .x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        // Pick 20 most significant bits from the linear congruential generator.
        let value: T = T::new((self.x >> 44) as i64) / T::from_f32(524287.5) - T::new(1);
        [value].into()
    }

    #[inline]
    fn set_hash(&mut self, hash: u32) {
        self.hash = hash;
    }
}
