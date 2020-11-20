#![deny(unsafe_code)]
#![no_std]
#![no_main]

use panic_halt as _;

use nb::block;

use cortex_m_rt::entry;
use embedded_hal::blocking::delay::DelayMs;
use embedded_hal::digital::v2::OutputPin;
use stm32f1xx_hal::{
    delay::Delay,
    gpio::{gpioa::PA15, Alternate, PushPull},
    pac,
    prelude::*,
    pwm::{Channel, Pwm, C1},
    timer::{Tim2PartialRemap1, Timer},
};

const CAT_SONG: [(char, u32); 24] = [
    ('g', 2),
    ('e', 2),
    ('e', 2),
    ('f', 2),
    ('d', 2),
    ('d', 2),
    ('c', 1),
    ('e', 1),
    ('g', 4),
    ('c', 1),
    ('e', 1),
    ('g', 4),
    ('g', 2),
    ('e', 2),
    ('e', 2),
    ('f', 2),
    ('d', 2),
    ('d', 2),
    ('c', 1),
    ('e', 1),
    ('g', 4),
    ('c', 1),
    ('e', 1),
    ('c', 4),
];

#[entry]
fn main() -> ! {
    // Get access to the core peripherals from the cortex-m crate
    let cp = cortex_m::Peripherals::take().unwrap();
    // Get access to the device specific peripherals from the peripheral access crate
    let dp = pac::Peripherals::take().unwrap();

    // Take ownership over the raw flash and rcc devices and convert them into the corresponding
    // HAL structs
    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();
    let mut afio = dp.AFIO.constrain(&mut rcc.apb2);
    // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
    // `clocks`
    let clocks = rcc.cfgr.freeze(&mut flash.acr);

    // Acquire the GPIOC peripheral
    let mut gpioc = dp.GPIOC.split(&mut rcc.apb2);
    let mut gpioa = dp.GPIOA.split(&mut rcc.apb2);
    let gpiob = dp.GPIOB.split(&mut rcc.apb2);

    let (pa15, _, _) = afio.mapr.disable_jtag(gpioa.pa15, gpiob.pb3, gpiob.pb4);
    let pwm_pin = pa15.into_alternate_push_pull(&mut gpioa.crh);

    // Configure gpio C pin 13 as a push-pull output. The `crh` register is passed to the function
    // in order to configure the port. For pins 0-7, crl should be passed instead.
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
    // Configure the syst timer to trigger an update every second
    //let mut timer = Timer::syst(cp.SYST, &clocks).start_count_down(1.hz());
    let mut delay = Delay::new(cp.SYST, clocks);
    let mut pwm = Timer::tim2(dp.TIM2, &clocks, &mut rcc.apb1).pwm::<Tim2PartialRemap1, _, _, _>(
        pwm_pin,
        &mut afio.mapr,
        1.khz(),
    );

    let mut tone = Tone::new(pwm);
    // Wait for the timer to trigger an update and change the state of the LED
    loop {
        delay.delay_ms(1000u32);
        led.set_high().unwrap();
        delay.delay_ms(1000u32);
        led.set_low().unwrap();

        tone.play_song(&CAT_SONG, &mut delay);
    }
}

//['c', 'd', 'e', 'f', 'g', 'a', 'b', 'C'];
// [262, 293, 329, 349, 392, 440, 494, 523];

type PwmTone = Pwm<pac::TIM2, Tim2PartialRemap1, C1, PA15<Alternate<PushPull>>>;

struct Tone {
    pwm: PwmTone,
    notes: [char; 8],
    frequencies: [u32; 8],
    tempo: u32,
}

impl Tone {
    fn new(mut pwm: PwmTone) -> Self {
        pwm.set_duty(Channel::C1, pwm.get_max_duty() / 2);
        Self {
            pwm,
            notes: ['c', 'd', 'e', 'f', 'g', 'a', 'b', 'C'],
            frequencies: [262, 293, 329, 349, 392, 440, 494, 523],
            tempo: 100,
        }
    }

    fn play_song<D: DelayMs<u32>>(&mut self, notes: &[(char, u32)], delay: &mut D) {
        self.pwm.enable(Channel::C1);
        for (note, beat) in notes.iter() {
            let tone_duration = beat * self.tempo;
            self.play_tone(note);
            delay.delay_ms(tone_duration);
        }
        self.pwm.disable(Channel::C1);
    }

    fn play_tone(&mut self, n: &char) {
        for (idx, note) in self.notes.iter().enumerate() {
            if note == n {
                self.pwm.set_period(self.frequencies[idx].hz())
            }
        }
    }
}
