// Copyright (C) 2026 Johannes Overmann and contributors.
// SPDX-License-Identifier: GPL-2.0-or-later
//! TED's two tone generators and the second generator's noise mode.
//!
//! The machine calls `advance` after each CPU instruction. Register writes
//! therefore change the waveform at the same instruction boundary as video
//! mode changes. Audio samples are collected independently of the GUI so the
//! chip can be tested without opening an output device.

/// The CPU executes 57 cycles on each of 312 PAL lines, 50 times per second.
const CPU_HZ: u64 = 57 * 312 * 50;
/// One tone counter tick takes four CPU cycles. Each full square wave takes
/// two counter periods. The 1023 setting wraps through a full 1024 counts.
const TONE_HZ: f64 = CPU_HZ as f64 / 8.0;

pub(crate) struct Sound {
    sample_rate: u32,
    sample_clock: u64,
    cycles_since_sample: u32,
    phases: [f64; 2],
    noise: u8,
    samples: Vec<f32>,
}

impl Default for Sound {
    fn default() -> Self {
        Self {
            sample_rate: 0,
            sample_clock: 0,
            cycles_since_sample: 0,
            phases: [0.0; 2],
            noise: 0xa5,
            samples: Vec::new(),
        }
    }
}

impl Sound {
    pub(crate) fn set_sample_rate(&mut self, rate: u32) {
        self.sample_rate = rate;
        self.sample_clock = 0;
        self.cycles_since_sample = 0;
        self.samples.clear();
    }

    pub(crate) fn reset(&mut self) {
        let rate = self.sample_rate;
        *self = Self::default();
        self.sample_rate = rate;
    }

    pub(crate) fn drain(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.samples)
    }

    pub(crate) fn advance(&mut self, cycles: u8, registers: &[u8; 64]) {
        if self.sample_rate == 0 {
            return;
        }
        // Integer clock arithmetic prevents long runs from drifting relative
        // to the PAL frame clock, even at rates such as 44,100 Hz.
        for _ in 0..cycles {
            self.cycles_since_sample += 1;
            self.sample_clock += self.sample_rate as u64;
            if self.sample_clock >= CPU_HZ {
                self.sample_clock -= CPU_HZ;
                let elapsed = std::mem::take(&mut self.cycles_since_sample);
                let sample = self.render_sample(elapsed, registers);
                self.samples.push(sample);
            }
        }
    }

    fn render_sample(&mut self, elapsed_cycles: u32, r: &[u8; 64]) -> f32 {
        let freqs = [
            u16::from(r[0x0e]) | (u16::from(r[0x12] & 3) << 8),
            u16::from(r[0x0f]) | (u16::from(r[0x10] & 3) << 8),
        ];
        let mut tone = [0.0f32; 2];
        for voice in 0..2 {
            let period = match freqs[voice] {
                1022 => 0,    // The counter stays high at this setting.
                1023 => 1024, // It wraps before the output changes state.
                value => 1023 - value,
            };
            if period == 0 {
                tone[voice] = 1.0;
                continue;
            }
            let old_phase = self.phases[voice];
            let next =
                old_phase + elapsed_cycles as f64 * TONE_HZ / (period as f64 * CPU_HZ as f64);
            self.phases[voice] = next.fract();
            tone[voice] = if self.phases[voice] < 0.5 { 1.0 } else { -1.0 };
            if voice == 1 {
                // The counter clocks the noise register on each half wave,
                // when it flips the square-wave bit.
                let shifts = (next * 2.0).floor() as u32 - (old_phase * 2.0).floor() as u32;
                for _ in 0..shifts {
                    let feedback = ((self.noise >> 7)
                        ^ (self.noise >> 5)
                        ^ (self.noise >> 4)
                        ^ (self.noise >> 3))
                        & 1;
                    self.noise = (self.noise << 1) | feedback;
                }
            }
        }
        let control = r[0x11];
        if control & 0x80 != 0 {
            // Bit 7 switches to the TED's direct volume DAC. Programs can
            // write successive volume values to play digital samples.
            return (f32::from((control & 15).min(8)) - 4.0) * 0.05;
        }
        let mut mixed = 0.0;
        if control & 0x10 != 0 {
            mixed += tone[0];
        }
        // TED gives voice two's tone precedence when both mode bits are set.
        if control & 0x20 != 0 {
            mixed += tone[1];
        } else if control & 0x40 != 0 {
            mixed += if self.noise & 1 != 0 { 1.0 } else { -1.0 };
        }
        // The analog TED output is mono. Two voices can sum to 0.9, while a
        // single voice at VOL 8 reaches 0.45 (about -7 dBFS).
        mixed * (f32::from((control & 15).min(8)) / 8.0) * 0.45
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate(sound: &mut Sound, r: &[u8; 64], cycles: usize) -> Vec<f32> {
        for _ in 0..cycles / 6 {
            sound.advance(6, r);
        }
        sound.drain()
    }

    #[test]
    fn samples_follow_pal_time_and_volume_control() {
        let mut sound = Sound::default();
        sound.set_sample_rate(48_000);
        let mut r = [0; 64];
        r[0x0e] = 0x02; // tone one, register value 770
        r[0x12] = 0x03;
        r[0x11] = 0x18;
        let samples = generate(&mut sound, &r, CPU_HZ as usize);
        assert_eq!(samples.len(), 48_000);
        assert!(samples.iter().any(|s| *s > 0.1));
        assert!(samples.iter().any(|s| *s < -0.1));
        assert_eq!(samples.iter().copied().fold(f32::MIN, f32::max), 0.45);
        let rising_edges = samples
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] > 0.0)
            .count();
        assert!((430..450).contains(&rising_edges), "{rising_edges} Hz");
        r[0x11] = 0x00;
        assert!(generate(&mut sound, &r, 6_000).iter().all(|s| *s == 0.0));
        r[0x11] = 0x88;
        assert!(generate(&mut sound, &r, 6_000).iter().all(|s| *s == 0.20));
    }

    #[test]
    fn reset_keeps_output_rate_and_clears_buffer() {
        let mut sound = Sound::default();
        sound.set_sample_rate(44_100);
        generate(&mut sound, &[0; 64], 600);
        sound.advance(60, &[0; 64]);
        sound.reset();
        assert!(sound.drain().is_empty());
        assert_eq!(
            generate(&mut sound, &[0; 64], CPU_HZ as usize).len(),
            44_100
        );
    }

    #[test]
    fn second_voice_noise_and_tone_priority() {
        let mut r = [0; 64];
        r[0x0f] = 0x02;
        r[0x10] = 0x03;
        r[0x11] = 0x48; // noise, full volume
        let mut sound = Sound::default();
        sound.set_sample_rate(48_000);
        let noise = generate(&mut sound, &r, 30_000);
        assert!(noise.iter().any(|&sample| sample > 0.0));
        assert!(noise.iter().any(|&sample| sample < 0.0));

        let mut tone_only = Sound::default();
        tone_only.set_sample_rate(48_000);
        r[0x11] = 0x28;
        let expected = generate(&mut tone_only, &r, 30_000);
        let mut both = Sound::default();
        both.set_sample_rate(48_000);
        r[0x11] = 0x68;
        assert_eq!(generate(&mut both, &r, 30_000), expected);
    }
}
