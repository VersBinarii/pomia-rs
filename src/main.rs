#![no_std]
#![no_main]

use panic_halt as _;

use embedded_hal::blocking::delay::DelayMs;
use embedded_hal::Pwm;
use stm32f1xx_hal::{prelude::*, pwm::Channel, stm32, time::Hertz};

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

#[rtic::app(device = crate::stm32)]
mod app {
    use rtic_core::prelude::TupleExt02;
    use stm32f1xx_hal::{
        delay::Delay,
        gpio::{gpioa::PA15, gpioc::PC13, Alternate, Output, PushPull},
        pac::{TIM2, TIM3},
        prelude::*,
        pwm::{Channel, Pwm, C1},
        timer::{CountDownTimer, Event, Tim2PartialRemap1, Timer},
    };

    #[resources]
    struct Resource {
        led: PC13<Output<PushPull>>,
        tim: CountDownTimer<TIM3>,
        pwm: Pwm<TIM2, Tim2PartialRemap1, C1, PA15<Alternate<PushPull>>>,
        delay: Delay,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        // Get access to the core peripherals from the cortex-m crate
        let cp = cx.core;
        // Get access to the device specific peripherals from the peripheral access crate
        let dp = cx.device;

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
        let led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
        let mut timer3 = Timer::tim3(dp.TIM3, &clocks, &mut rcc.apb1).start_count_down(1.hz());
        timer3.listen(Event::Update);

        // Configure the syst timer to trigger an update every second
        //let mut timer = Timer::syst(cp.SYST, &clocks).start_count_down(1.hz());
        let delay = Delay::new(cp.SYST, clocks);
        let pwm = Timer::tim2(dp.TIM2, &clocks, &mut rcc.apb1).pwm::<Tim2PartialRemap1, _, _, _>(
            pwm_pin,
            &mut afio.mapr,
            1.khz(),
        );

        init::LateResources {
            led,
            tim: timer3,
            pwm,
            delay,
        }
    }

    #[idle(resources = [pwm, delay])]
    fn idle(cx: idle::Context) -> ! {
        let pwm = cx.resources.pwm;
        let delay = cx.resources.delay;
        (pwm, delay).lock(|pwm, delay| {
            let mut tone = crate::Tone::new(pwm, Channel::C1);
            loop {
                tone.play_song(&crate::CAT_SONG, delay);
            }
        });
        loop {}
    }

    #[task(binds = TIM3, resources = [led, tim])]
    fn tim3(mut cx: tim3::Context) {
        let _ = cx.resources.led.lock(|led| led.toggle());
        let _ = cx.resources.tim.lock(|tim| tim.wait());
    }
}

//['c', 'd', 'e', 'f', 'g', 'a', 'b', 'C'];
// [262, 293, 329, 349, 392, 440, 494, 523];

struct Tone<'a, P> {
    pwm: &'a mut P,
    notes: [char; 8],
    frequencies: [u32; 8],
    tempo: u32,
    channel: Channel,
}

impl<'a, P> Tone<'a, P>
where
    P: Pwm<Channel = Channel, Duty = u16, Time = Hertz>,
{
    fn new(pwm: &'a mut P, channel: Channel) -> Self {
        pwm.set_duty(channel, pwm.get_max_duty() / 2);
        Self {
            pwm,
            notes: ['c', 'd', 'e', 'f', 'g', 'a', 'b', 'C'],
            frequencies: [262, 293, 329, 349, 392, 440, 494, 523],
            tempo: 100,
            channel,
        }
    }

    fn play_song<D: DelayMs<u32>>(&mut self, notes: &[(char, u32)], delay: &mut D) {
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
