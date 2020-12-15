use embedded_hal::{blocking::delay::DelayMs, Pwm};
use stm32f1xx_hal::{prelude::*, pwm::Channel, time::Hertz};

//['c', 'd', 'e', 'f', 'g', 'a', 'b', 'C'];
// [262, 293, 329, 349, 392, 440, 494, 523];

pub struct Tone<P> {
    pwm: P,
    notes: [char; 8],
    frequencies: [u32; 8],
    tempo: u32,
    channel: Channel,
}

impl<P> Tone<P>
where
    P: Pwm<Channel = Channel, Duty = u16, Time = Hertz>,
{
    pub fn new(mut pwm: P, channel: Channel) -> Self {
        pwm.set_duty(channel, pwm.get_max_duty() / 2);
        Self {
            pwm,
            notes: ['c', 'd', 'e', 'f', 'g', 'a', 'b', 'C'],
            frequencies: [262, 293, 329, 349, 392, 440, 494, 523],
            tempo: 100,
            channel,
        }
    }

    pub fn play_song<D: DelayMs<u32>>(&mut self, notes: &[(char, u32)], delay: &mut D) {
        self.pwm.enable(self.channel);
        for (note, beat) in notes.iter() {
            let tone_duration = beat * self.tempo;
            self.play_tone(note);
            delay.delay_ms(tone_duration);
        }
        self.pwm.disable(self.channel);
    }

    fn play_tone(&mut self, n: &char) {
        for (idx, note) in self.notes.iter().enumerate() {
            if note == n {
                self.pwm.set_period(self.frequencies[idx].hz())
            }
        }
    }
}
