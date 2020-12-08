#![no_std]
#![no_main]

use panic_halt as _;

use embedded_hal::blocking::delay::DelayMs;
use embedded_hal::Pwm;
use stm32f1xx_hal::{stm32, time::Hertz};

use embedded_graphics::{
    fonts::{Font6x8, Text},
    pixelcolor::Rgb565,
    prelude::*,
    style::{MonoTextStyle, MonoTextStyleBuilder},
};
use st7735_lcd::ST7735;
use stm32f1xx_hal::{
    gpio::{
        gpioa::{PA5, PA6, PA7},
        gpiob::{PB0, PB1},
        Alternate, Floating, Input, Output, PushPull,
    },
    pac::SPI1,
    prelude::*,
    pwm::Channel,
    spi::{Spi, Spi1NoRemap},
};
type RESET = PB0<Output<PushPull>>;
type DC = PB1<Output<PushPull>>;
type SCK = PA5<Alternate<PushPull>>;
type MISO = PA6<Input<Floating>>;
type MOSI = PA7<Alternate<PushPull>>;
type DISP = ST7735<Spi<SPI1, Spi1NoRemap, (SCK, MISO, MOSI), u8>, DC, RESET>;

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

    use core::fmt::Write;
    use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
    use heapless::consts::*;
    use heapless::String;
    use rtic_core::prelude::*;
    use st7735_lcd::{Orientation, ST7735};
    use stm32f1xx_hal::{
        delay::Delay,
        gpio::{gpioa::PA15, gpioc::PC13, Alternate, Output, PushPull},
        pac::{TIM2, TIM3},
        prelude::*,
        pwm::{Channel, Pwm, C1},
        spi::{Mode, Phase, Polarity, Spi},
        timer::{CountDownTimer, Event, Tim2PartialRemap1, Timer},
    };

    #[resources]
    struct Resource {
        led: PC13<Output<PushPull>>,
        tim: CountDownTimer<TIM3>,
        tone: crate::Tone<Pwm<TIM2, Tim2PartialRemap1, C1, PA15<Alternate<PushPull>>>>,
        delay: Delay,
        display: crate::Display,
        #[init(0)]
        counter: u32,
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
        let clocks = rcc
            .cfgr
            .use_hse(8.mhz())
            .sysclk(72.mhz())
            .pclk1(36.mhz())
            .freeze(&mut flash.acr);

        // Acquire the GPIOC peripheral
        let mut gpioc = dp.GPIOC.split(&mut rcc.apb2);
        let mut gpioa = dp.GPIOA.split(&mut rcc.apb2);
        let mut gpiob = dp.GPIOB.split(&mut rcc.apb2);

        let (pa15, _, _) = afio.mapr.disable_jtag(gpioa.pa15, gpiob.pb3, gpiob.pb4);
        let pwm_pin = pa15.into_alternate_push_pull(&mut gpioa.crh);

        // Configure gpio C pin 13 as a push-pull output. The `crh` register is passed to the function
        // in order to configure the port. For pins 0-7, crl should be passed instead.
        let led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
        let mut timer3 = Timer::tim3(dp.TIM3, &clocks, &mut rcc.apb1).start_count_down(50.hz());
        timer3.listen(Event::Update);

        // Configure the syst timer to trigger an update every second
        //let mut timer = Timer::syst(cp.SYST, &clocks).start_count_down(1.hz());
        let mut delay = Delay::new(cp.SYST, clocks);
        let pwm = Timer::tim2(dp.TIM2, &clocks, &mut rcc.apb1).pwm::<Tim2PartialRemap1, _, _, _>(
            pwm_pin,
            &mut afio.mapr,
            1.khz(),
        );
        let tone = crate::Tone::new(pwm, Channel::C1);
        //SPI
        let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
        let miso = gpioa.pa6;
        let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);

        let rst = gpiob.pb0.into_push_pull_output(&mut gpiob.crl);
        let dc = gpiob.pb1.into_push_pull_output(&mut gpiob.crl);

        let spi = Spi::spi1(
            dp.SPI1,
            (sck, miso, mosi),
            &mut afio.mapr,
            Mode {
                polarity: Polarity::IdleLow,
                phase: Phase::CaptureOnFirstTransition,
            },
            16.mhz(),
            clocks,
            &mut rcc.apb2,
        );

        let mut disp = ST7735::new(spi, dc, rst, true, false, 128, 160);

        disp.init(&mut delay).unwrap();
        disp.set_orientation(&Orientation::Portrait).unwrap();
        let _ = disp.clear(Rgb565::BLACK);
        let display = crate::Display::new(disp);
        init::LateResources {
            led,
            tim: timer3,
            tone,
            delay,
            display,
        }
    }

    #[idle(resources = [tone, delay, display, counter])]
    fn idle(cx: idle::Context) -> ! {
        let tone = cx.resources.tone;
        let delay = cx.resources.delay;
        let display = cx.resources.display;
        let mut counter = cx.resources.counter;
        (tone, delay, display).lock(|tone, delay, display| {
            // draw stuff here
            display.print_text("We're printing", 10, 30);
            tone.play_song(&crate::CAT_SONG, delay);
            loop {
                let mut text: String<U16> = String::new();
                let counter = counter.lock(|c| *c);
                write!(text, "Counter: {}", counter).unwrap();
                display.print_text(&text, 10, 40);
            }
        });
        loop {}
    }

    #[task(binds = TIM3, resources = [led, tim, counter])]
    fn tim3(mut cx: tim3::Context) {
        let _ = cx.resources.led.lock(|led| led.toggle());
        let _ = cx.resources.tim.lock(|tim| tim.wait());
        let _ = cx.resources.counter.lock(|c| *c += 1);
    }
}

pub struct Display {
    style: MonoTextStyle<Rgb565, Font6x8>,
    display: DISP,
}

impl Display {
    pub fn new(display: DISP) -> Self {
        let style = MonoTextStyleBuilder::new(Font6x8)
            .text_color(Rgb565::RED)
            .background_color(Rgb565::BLACK)
            .build();

        Self { display, style }
    }

    pub fn print_text(&mut self, text: &str, x: i32, y: i32) {
        Text::new(text, Point::new(x, y))
            .into_styled(self.style)
            .draw(&mut self.display)
            .unwrap();
    }
}

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
