use super::audionode::*;
use super::lti::*;
use super::math::*;
use super::*;
use num_complex::Complex64;
use numeric_array::*;

/// Complex64 with real component `x` and imaginary component zero.
fn re<T: Float>(x: T) -> Complex64 {
    Complex64::new(x.to_f64(), 0.0)
}

#[derive(Copy, Clone, Debug)]
pub struct BiquadCoefs<F> {
    a1: F,
    a2: F,
    b0: F,
    b1: F,
    b2: F,
}

impl<F: Real> BiquadCoefs<F> {
    /// Returns settings for a Butterworth lowpass filter.
    /// Cutoff is the -3 dB point of the filter in Hz.
    pub fn butter_lowpass(sample_rate: F, cutoff: F) -> BiquadCoefs<F> {
        let c = F::from_f64;
        let f: F = tan(cutoff * c(PI) / sample_rate);
        let a0r: F = c(1.0) / (c(1.0) + c(SQRT_2) * f + f * f);
        let a1: F = (c(2.0) * f * f - c(2.0)) * a0r;
        let a2: F = (c(1.0) - c(SQRT_2) * f + f * f) * a0r;
        let b0: F = f * f * a0r;
        let b1: F = c(2.0) * b0;
        let b2: F = b0;
        BiquadCoefs::<F> { a1, a2, b0, b1, b2 }
    }

    /// Returns settings for a constant-gain bandpass resonator.
    /// The center frequency is given in Hz.
    /// Bandwidth is the difference in Hz between -3 dB points of the filter response.
    /// The overall gain of the filter is independent of bandwidth.
    pub fn resonator(sample_rate: F, center: F, bandwidth: F) -> BiquadCoefs<F> {
        let c = F::from_f64;
        let r: F = exp(c(-PI) * bandwidth / sample_rate);
        let a1: F = c(-2.0) * r * cos(c(TAU) * center / sample_rate);
        let a2: F = r * r;
        let b0: F = sqrt(c(1.0) - r * r) * c(0.5);
        let b1: F = c(0.0);
        let b2: F = -b0;
        BiquadCoefs::<F> { a1, a2, b0, b1, b2 }
    }
}

impl<F: Real> Lti for BiquadCoefs<F> {
    fn response(&self, omega: f64) -> Complex64 {
        let e1 = Complex64::from_polar(1.0, -TAU * omega);
        let e2 = Complex64::from_polar(1.0, -2.0 * TAU * omega);
        (re(self.b0) + re(self.b1) * e1 + re(self.b2) * e2)
            / (re(1.0) + re(self.a1) * e1 + re(self.a2) * e2)
    }
}

/// 2nd order IIR filter implemented in normalized Direct Form I.
#[derive(Copy, Clone, Default)]
pub struct Biquad<T, F> {
    _marker: std::marker::PhantomData<T>,
    a1: F,
    a2: F,
    b0: F,
    b1: F,
    b2: F,
    x1: F,
    x2: F,
    y1: F,
    y2: F,
}

impl<T: Float, F: Real> Biquad<T, F> {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn set_coefs(&mut self, coefs: BiquadCoefs<F>) {
        self.a1 = coefs.a1;
        self.a2 = coefs.a2;
        self.b0 = coefs.b0;
        self.b1 = coefs.b1;
        self.b2 = coefs.b2;
    }
}

impl<T: Float, F: Real> AudioNode for Biquad<T, F> {
    const ID: u32 = 15;
    type Sample = T;
    type Inputs = typenum::U1;
    type Outputs = typenum::U1;

    fn reset(&mut self, _sample_rate: Option<f64>) {
        self.x1 = F::zero();
        self.x2 = F::zero();
        self.y1 = F::zero();
        self.y2 = F::zero();
    }

    #[inline]
    fn tick(
        &mut self,
        input: &Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let x0 = convert(input[0]);
        let y0 = self.b0 * x0 + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x0;
        self.y2 = self.y1;
        self.y1 = y0;
        [convert(y0)].into()

        // Transposed Direct Form II would be:
        //   y0 = b0 * x0 + s1
        //   s1 = s2 + b1 * x0 - a1 * y0
        //   s2 = b2 * x0 - a2 * y0
    }
}

/// Butterworth lowpass filter.
/// - Input 0: input signal
/// - Input 1: cutoff frequency (Hz)
/// - Output 0: filtered signal
#[derive(Copy, Clone)]
pub struct ButterLowpass<T: Float, F: Real> {
    biquad: Biquad<T, F>,
    sample_rate: F,
    cutoff: F,
}

impl<T: Float, F: Real> ButterLowpass<T, F> {
    // TODO: f64 for sample_rate?
    pub fn new(sample_rate: F) -> ButterLowpass<T, F> {
        ButterLowpass {
            biquad: Biquad::new(),
            sample_rate,
            cutoff: F::zero(),
        }
    }
}

impl<T: Float, F: Real> AudioNode for ButterLowpass<T, F> {
    const ID: u32 = 16;
    type Sample = T;
    type Inputs = typenum::U2;
    type Outputs = typenum::U1;

    fn reset(&mut self, sample_rate: Option<f64>) {
        self.biquad.reset(sample_rate);
        self.cutoff = F::zero();
    }

    #[inline]
    fn tick(
        &mut self,
        input: &Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let cutoff: F = convert(input[1]);
        if cutoff != self.cutoff {
            self.biquad
                .set_coefs(BiquadCoefs::butter_lowpass(self.sample_rate, cutoff));
            self.cutoff = cutoff;
        }
        self.biquad.tick(&[input[0]].into())
    }
}

/// Constant-gain bandpass filter (resonator).
/// Filter gain is (nearly) independent of bandwidth.
/// - Input 0: input signal
/// - Input 1: filter center frequency (peak) (Hz)
/// - Input 2: filter bandwidth (distance) between -3 dB points (Hz)
/// - Output 0: filtered signal
#[derive(Copy, Clone)]
pub struct Resonator<T: Float, F: Real> {
    biquad: Biquad<T, F>,
    sample_rate: F,
    center: F,
    bandwidth: F,
}

impl<T: Float, F: Real> Resonator<T, F> {
    pub fn new(sample_rate: F) -> Resonator<T, F> {
        Resonator {
            biquad: Biquad::new(),
            sample_rate,
            center: F::zero(),
            bandwidth: F::zero(),
        }
    }
}

impl<T: Float, F: Real> AudioNode for Resonator<T, F> {
    const ID: u32 = 17;
    type Sample = T;
    type Inputs = typenum::U3;
    type Outputs = typenum::U1;

    fn reset(&mut self, sample_rate: Option<f64>) {
        self.biquad.reset(sample_rate);
        if let Some(sr) = sample_rate {
            self.sample_rate = convert(sr);
        }
        self.center = F::zero();
        self.bandwidth = F::zero();
    }

    #[inline]
    fn tick(
        &mut self,
        input: &Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let center: F = convert(input[1]);
        let bandwidth: F = convert(input[2]);
        if center != self.center || bandwidth != self.bandwidth {
            self.biquad
                .set_coefs(BiquadCoefs::resonator(self.sample_rate, center, bandwidth));
            self.center = center;
            self.bandwidth = bandwidth;
        }
        self.biquad.tick(&[input[0]].into())
    }
}

/// One-pole lowpass filter.
/// - Input 0: input signal
/// - Input 1: cutoff frequency (Hz)
/// - Output 0: filtered signal
#[derive(Copy, Clone, Default)]
pub struct OnePoleLowpass<T: Float, F: Real> {
    _marker: std::marker::PhantomData<T>,
    value: F,
    coeff: F,
    cutoff: F,
    sample_rate: F,
}

impl<T: Float, F: Real> OnePoleLowpass<T, F> {
    pub fn new(sample_rate: f64) -> Self {
        OnePoleLowpass {
            _marker: std::marker::PhantomData,
            value: F::zero(),
            coeff: F::zero(),
            cutoff: F::zero(),
            sample_rate: convert(sample_rate),
        }
    }
}

impl<T: Float, F: Real> AudioNode for OnePoleLowpass<T, F> {
    const ID: u32 = 18;
    type Sample = T;
    type Inputs = typenum::U2;
    type Outputs = typenum::U1;

    fn reset(&mut self, sample_rate: Option<f64>) {
        if let Some(sample_rate) = sample_rate {
            self.sample_rate = convert(sample_rate);
            self.cutoff = F::zero();
        }
        self.value = F::zero();
    }

    #[inline]
    fn tick(
        &mut self,
        input: &Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let cutoff: F = convert(input[1]);
        if cutoff != self.cutoff {
            self.cutoff = cutoff;
            self.coeff = exp(F::from_f64(-TAU) * cutoff / self.sample_rate);
        }
        let x = convert(input[0]);
        self.value = (F::one() - self.coeff) * x + self.coeff * self.value;
        [convert(self.value)].into()
    }
}

/// DC blocking filter.
/// - Input 0: input signal
/// - Output 0: zero centered signal
#[derive(Copy, Clone, Default)]
pub struct DCBlocker<T: Float, F: Real> {
    _marker: std::marker::PhantomData<T>,
    x1: F,
    y1: F,
    cutoff: F,
    coeff: F,
}

impl<T: Float, F: Real> DCBlocker<T, F> {
    pub fn new(sample_rate: f64, cutoff: F) -> Self {
        let mut node = DCBlocker::default();
        node.cutoff = cutoff;
        node.reset(Some(sample_rate));
        node
    }
}

impl<T: Float, F: Real> AudioNode for DCBlocker<T, F> {
    const ID: u32 = 22;
    type Sample = T;
    type Inputs = typenum::U1;
    type Outputs = typenum::U1;

    fn reset(&mut self, sample_rate: Option<f64>) {
        if let Some(sample_rate) = sample_rate {
            self.coeff = F::one() - (F::from_f64(TAU / sample_rate) * self.cutoff);
        }
        self.x1 = F::zero();
        self.y1 = F::zero();
    }

    #[inline]
    fn tick(
        &mut self,
        input: &Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let x = convert(input[0]);
        let y0 = x - self.x1 + self.coeff * self.y1;
        self.x1 = x;
        self.y1 = y0;
        [convert(y0)].into()
    }
}

/// Transient filter. Multiply the signal with a fade-in curve.
/// After fade-in, pass signal through.
/// - Input 0: input signal
/// - Output 0: filtered signal
#[derive(Copy, Clone, Default)]
pub struct Declicker<T: Float, F: Real> {
    _marker: std::marker::PhantomData<T>,
    t: F,
    duration: F,
    sample_duration: F,
}

impl<T: Float, F: Real> Declicker<T, F> {
    pub fn new(sample_rate: f64, duration: F) -> Self {
        let mut node = Declicker::default();
        node.duration = duration;
        node.reset(Some(sample_rate));
        node
    }
}

impl<T: Float, F: Real> AudioNode for Declicker<T, F> {
    const ID: u32 = 22;
    type Sample = T;
    type Inputs = typenum::U1;
    type Outputs = typenum::U1;

    fn reset(&mut self, sample_rate: Option<f64>) {
        if let Some(sample_rate) = sample_rate {
            self.sample_duration = F::from_f64(1.0 / sample_rate);
        }
        self.t = F::zero();
    }

    #[inline]
    fn tick(
        &mut self,
        input: &Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        if self.t < self.duration {
            let phase = delerp(F::zero(), self.duration, self.t);
            let value = smooth9(phase);
            self.t = self.t + self.sample_duration;
            [input[0] * convert(value)].into()
        } else {
            [input[0]].into()
        }
    }
}
