#![no_std]
#![no_main]

mod display;
mod tone;

use panic_halt as _;

use stm32f1xx_hal::stm32;

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

    use crate::display::{Clock, Display, Gui};
    use crate::tone::Tone;
    use bme280::BME280;
    use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
    use embedded_hal::digital::v2::InputPin;
    use rtic_core::prelude::*;
    use st7735_lcd::{Orientation, ST7735};
    use stm32f1xx_hal::{
        delay::Delay,
        gpio::{
            gpioa::{PA0, PA11, PA12, PA15},
            gpiob::{PB8, PB9},
            gpioc::PC13,
            Alternate, Edge, ExtiPin, Input, OpenDrain, Output, PullUp, PushPull,
        },
        i2c::{BlockingI2c, DutyCycle, Mode as I2cMode},
        pac::{I2C1, TIM2, TIM3},
        prelude::*,
        pwm::{Channel, Pwm, C1},
        rtc::Rtc,
        spi::{Mode as SpiMode, Phase, Polarity, Spi},
        timer::{CountDownTimer, Event, Tim2NoRemap, Timer},
    };
    use ufmt::derive::uDebug;

    type SCL = PB8<Alternate<OpenDrain>>;
    type SDA = PB9<Alternate<OpenDrain>>;

    #[derive(uDebug, Copy, Clone)]
    pub enum PressedButton {
        Left,
        Right,
        ShortPress,
        LongPress,
        None,
    }

    pub struct Buttons {
        enter: PA15<Input<PullUp>>,
        left: PA11<Input<PullUp>>,
        right: PA12<Input<PullUp>>,
    }

    #[resources]
    struct Resource {
        led: PC13<Output<PushPull>>,
        tim: CountDownTimer<TIM3>,
        tone: Tone<Pwm<TIM2, Tim2NoRemap, C1, PA0<Alternate<PushPull>>>>,
        delay: Delay,
        bme: BME280<BlockingI2c<I2C1, (SCL, SDA)>>,
        buttons: Buttons,
        gui: Gui,
        clock: Clock,
        #[init(PressedButton::None)]
        pressed_btn: PressedButton,
        #[init(0)]
        press_counter: u8,
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

        // Configure gpio C pin 13 as a push-pull output. The `crh` register is passed to the function
        // in order to configure the port. For pins 0-7, crl should be passed instead.
        let led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
        let mut timer3 = Timer::tim3(dp.TIM3, &clocks, &mut rcc.apb1).start_count_down(2.hz());
        timer3.listen(Event::Update);

        // PWM config
        let pwm_pin = gpioa.pa0.into_alternate_push_pull(&mut gpioa.crl);
        let mut delay = Delay::new(cp.SYST, clocks);
        let pwm = Timer::tim2(dp.TIM2, &clocks, &mut rcc.apb1).pwm::<Tim2NoRemap, _, _, _>(
            pwm_pin,
            &mut afio.mapr,
            1.khz(),
        );
        let tone = Tone::new(pwm, Channel::C1);

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
            SpiMode {
                polarity: Polarity::IdleLow,
                phase: Phase::CaptureOnFirstTransition,
            },
            16.mhz(),
            clocks,
            &mut rcc.apb2,
        );

        // Instanciate Display driver
        let mut disp = ST7735::new(spi, dc, rst, true, false, 128, 160);

        disp.init(&mut delay).unwrap();
        disp.set_orientation(&Orientation::Portrait).unwrap();
        let _ = disp.clear(Rgb565::BLACK);
        let display = Display::new(disp);

        let gui = Gui::new(display);

        // I2C config
        let scl = gpiob.pb8.into_alternate_open_drain(&mut gpiob.crh);
        let sda = gpiob.pb9.into_alternate_open_drain(&mut gpiob.crh);
        let i2c = BlockingI2c::i2c1(
            dp.I2C1,
            (scl, sda),
            &mut afio.mapr,
            I2cMode::Fast {
                frequency: 400000.hz(),
                duty_cycle: DutyCycle::Ratio2to1,
            },
            clocks,
            &mut rcc.apb1,
            5000,
            3,
            5000,
            5000,
        );

        //Initialize the sensor
        let mut bme = BME280::new_primary(i2c);
        let _ = bme.init(&mut delay);

        let (pa15, _, _) = afio.mapr.disable_jtag(gpioa.pa15, gpiob.pb3, gpiob.pb4);
        let mut enter = pa15.into_pull_up_input(&mut gpioa.crh);
        enter.make_interrupt_source(&mut afio);
        enter.trigger_on_edge(&dp.EXTI, Edge::RISING_FALLING);
        enter.enable_interrupt(&dp.EXTI);
        let mut left = gpioa.pa11.into_pull_up_input(&mut gpioa.crh); // PA11
        left.make_interrupt_source(&mut afio);
        left.trigger_on_edge(&dp.EXTI, Edge::FALLING);
        left.enable_interrupt(&dp.EXTI);
        let mut right = gpioa.pa12.into_pull_up_input(&mut gpioa.crh); //PA12
        right.make_interrupt_source(&mut afio);
        right.trigger_on_edge(&dp.EXTI, Edge::FALLING);
        right.enable_interrupt(&dp.EXTI);

        let buttons = Buttons { enter, left, right };

        // Initialize RTC
        let mut pwr = dp.PWR;
        let mut backup_domain = rcc.bkp.constrain(dp.BKP, &mut rcc.apb1, &mut pwr);
        let rtc = Rtc::rtc(dp.RTC, &mut backup_domain);
        let clock = Clock::new(rtc);

        init::LateResources {
            led,
            tim: timer3,
            tone,
            delay,
            bme,
            gui,
            buttons,
            clock,
        }
    }

    #[idle(resources = [tone, delay, bme, gui, clock, pressed_btn])]
    fn idle(cx: idle::Context) -> ! {
        let tone = cx.resources.tone;
        let delay = cx.resources.delay;
        let bme = cx.resources.bme;
        let mut gui = cx.resources.gui;
        let clock = cx.resources.clock;
        let mut pressed_btn = cx.resources.pressed_btn;
        (tone, delay, bme, clock).lock(|tone, delay, bme, clock| {
            // draw stuff here
            tone.play_song(&crate::CAT_SONG, delay);
            loop {
                // Update buttons
                let pb = pressed_btn.lock(|pb| *pb);
                match pb {
                    PressedButton::Left => {
                        gui.lock(|g| g.backward());
                        pressed_btn.lock(|pb| *pb = PressedButton::None);
                    }
                    PressedButton::Right => {
                        gui.lock(|g| g.forward());
                        pressed_btn.lock(|pb| *pb = PressedButton::None);
                    }
                    _ => {}
                };

                // Update screen

                gui.lock(|g| {
                    g.print_header();

                    match bme.measure(delay) {
                        Ok(measurement) => {
                            g.print_measurements((
                                measurement.temperature as u8,
                                measurement.humidity as u8,
                                measurement.pressure as u32,
                            ));
                        }
                        Err(e) => g.print_error(&e),
                    };

                    g.print_clock(&clock);
                });
            }
        });
        loop {}
    }

    #[task(binds = EXTI15_10, resources = [buttons, press_counter, pressed_btn])]
    fn exti15_10(cx: exti15_10::Context) {
        let buttons = cx.resources.buttons;
        let press_counter = cx.resources.press_counter;
        let pressed_btn = cx.resources.pressed_btn;

        (buttons, press_counter, pressed_btn).lock(|buttons, pc, pb| {
            let Buttons { enter, left, right } = buttons;
            if enter.check_interrupt() {
                if enter.is_low().unwrap() {
                    *pc = 0;
                } else if enter.is_high().unwrap() && *pc > 3 {
                    *pb = PressedButton::LongPress;
                } else if enter.is_high().unwrap() && *pc <= 3 {
                    *pb = PressedButton::ShortPress;
                }
            } else if left.check_interrupt() && left.is_low().unwrap() {
                *pb = PressedButton::Left;
            } else if right.check_interrupt() && right.is_low().unwrap() {
                *pb = PressedButton::Right;
            }

            enter.clear_interrupt_pending_bit();
            left.clear_interrupt_pending_bit();
            right.clear_interrupt_pending_bit();
        })
    }

    #[task(binds = TIM3, resources = [led, tim, press_counter])]
    fn tim3(mut cx: tim3::Context) {
        let _ = cx.resources.led.lock(|led| led.toggle());
        let _ = cx.resources.tim.lock(|tim| tim.wait());
        cx.resources.press_counter.lock(|pc| *pc += 1);
    }
}
