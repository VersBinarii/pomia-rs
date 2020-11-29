#![no_std]
#![no_main]

use panic_halt as _;

use core::cell::RefCell;
use cortex_m::interrupt::Mutex;
use cortex_m_rt::entry;
use embedded_hal::blocking::delay::DelayMs;
use embedded_hal::Pwm;
use stm32f1xx_hal::{
    delay::Delay,
    gpio::{gpioc::PC13, Output, PushPull},
    pac::{self, interrupt, TIM3},
    prelude::*,
    pwm::Channel,
    time::Hertz,
    timer::{CountDownTimer, Event, Tim2PartialRemap1, Timer},
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

// Make LED pin globally available
static G_LED: Mutex<RefCell<Option<PC13<Output<PushPull>>>>> = Mutex::new(RefCell::new(None));
// Make timer interrupt registers globally available
static G_TIM: Mutex<RefCell<Option<CountDownTimer<TIM3>>>> = Mutex::new(RefCell::new(None));

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
    let led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
    cortex_m::interrupt::free(|cs| {
        *G_LED.borrow(cs).borrow_mut() = Some(led);
    });
    let mut timer3 = Timer::tim3(dp.TIM3, &clocks, &mut rcc.apb1).start_count_down(1.hz());
    timer3.listen(Event::Update);
    cortex_m::interrupt::free(|cs| {
        *G_TIM.borrow(cs).borrow_mut() = Some(timer3);
    });
    // Configure the syst timer to trigger an update every second
    //let mut timer = Timer::syst(cp.SYST, &clocks).start_count_down(1.hz());
    let mut delay = Delay::new(cp.SYST, clocks);
    let pwm = Timer::tim2(dp.TIM2, &clocks, &mut rcc.apb1).pwm::<Tim2PartialRemap1, _, _, _>(
        pwm_pin,
        &mut afio.mapr,
        1.khz(),
    );

    let mut tone = Tone::new(pwm, Channel::C1);
    // Wait for the timer to trigger an update and change the state of the LED
    loop {
        tone.play_song(&CAT_SONG, &mut delay);
    }
}

#[interrupt]
unsafe fn TIM3() {
    static mut LED: Option<PC13<Output<PushPull>>> = None;
    static mut TIM: Option<CountDownTimer<TIM3>> = None;

    let led = LED.get_or_insert_with(|| {
        cortex_m::interrupt::free(|cs| {
            // Move LED pin here, leaving a None in its place
            G_LED.borrow(cs).replace(None).unwrap()
        })
    });

    let tim = TIM.get_or_insert_with(|| {
        cortex_m::interrupt::free(|cs| {
            // Move LED pin here, leaving a None in its place
            G_TIM.borrow(cs).replace(None).unwrap()
        })
    });

    let _ = led.toggle();
    let _ = tim.wait();
}
//['c', 'd', 'e', 'f', 'g', 'a', 'b', 'C'];
// [262, 293, 329, 349, 392, 440, 494, 523];

struct Tone<P> {
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
    fn new(mut pwm: P, channel: Channel) -> Self {
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
