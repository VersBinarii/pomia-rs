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

    use crate::display::{Display, Menu, MenuItem};
    use crate::tone::Tone;
    use bme280::BME280;
    use core::fmt::Write;
    use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
    use embedded_hal::digital::v2::InputPin;
    use heapless::{consts::*, String};
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
        spi::{Mode as SpiMode, Phase, Polarity, Spi},
        timer::{CountDownTimer, Event, Tim2NoRemap, Timer},
    };

    type SCL = PB8<Alternate<OpenDrain>>;
    type SDA = PB9<Alternate<OpenDrain>>;

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
        display: Display,
        bme: BME280<BlockingI2c<I2C1, (SCL, SDA)>>,
        buttons: Buttons,
        menu: Menu,
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
        let mut timer3 = Timer::tim3(dp.TIM3, &clocks, &mut rcc.apb1).start_count_down(50.hz());
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
        left.trigger_on_edge(&dp.EXTI, Edge::RISING_FALLING);
        left.enable_interrupt(&dp.EXTI);
        let mut right = gpioa.pa12.into_pull_up_input(&mut gpioa.crh); //PA12
        right.make_interrupt_source(&mut afio);
        right.trigger_on_edge(&dp.EXTI, Edge::RISING_FALLING);
        right.enable_interrupt(&dp.EXTI);

        let menu = Menu::new();

        let buttons = Buttons { enter, left, right };
        init::LateResources {
            led,
            tim: timer3,
            tone,
            delay,
            display,
            bme,
            menu,
            buttons,
        }
    }

    #[idle(resources = [tone, delay, display, bme, menu])]
    fn idle(cx: idle::Context) -> ! {
        let tone = cx.resources.tone;
        let delay = cx.resources.delay;
        let display = cx.resources.display;
        let bme = cx.resources.bme;
        let mut menu = cx.resources.menu;
        (tone, delay, display, bme).lock(|tone, delay, display, bme| {
            // draw stuff here
            tone.play_song(&crate::CAT_SONG, delay);
            loop {
                let mut text: String<U64> = String::new();

                let menu_item = menu.lock(|m| m.current_menu_item());
                display.render_tab_header(menu_item);
                text.clear();

                match bme.measure(delay) {
                    Ok(measurement) => {
                        let confirm = menu.lock(|m| m.confirm);
                        match menu_item {
                            MenuItem::Temperature if confirm => {
                                write!(text, "T: {:.2}C", measurement.temperature,).unwrap();
                                display.print_text(&text, 10, 22);
                            }
                            MenuItem::Humidity if confirm => {
                                write!(text, "H: {:.1}%", measurement.humidity,).unwrap();
                                display.print_text(&text, 10, 22);
                            }
                            MenuItem::Pressure if confirm => {
                                write!(text, "P: {:.0}Pa", measurement.pressure,).unwrap();
                                display.print_text(&text, 10, 22);
                            }
                            _ => {}
                        }
                        menu.lock(|m| m.confirm = false);
                    }
                    Err(e) => {
                        write!(text, "E: {:?}", e).unwrap();
                        display.print_text(&text, 10, 100);
                    }
                };
            }
        });
        loop {}
    }

    #[task(binds = EXTI15_10, resources = [buttons, menu])]
    fn exti15_10(cx: exti15_10::Context) {
        let buttons = cx.resources.buttons;
        let menu = cx.resources.menu;

        (buttons, menu).lock(|buttons, menu| {
            let Buttons { enter, left, right } = buttons;
            if enter.check_interrupt() && enter.is_low().unwrap() {
                menu.confirm = true;
            } else if left.check_interrupt() && left.is_low().unwrap() {
                menu.backward();
            } else if right.check_interrupt() && right.is_low().unwrap() {
                menu.forward();
            }

            enter.clear_interrupt_pending_bit();
            left.clear_interrupt_pending_bit();
            right.clear_interrupt_pending_bit();
        })
    }

    #[task(binds = TIM3, resources = [led, tim])]
    fn tim3(mut cx: tim3::Context) {
        let _ = cx.resources.led.lock(|led| led.toggle());
        let _ = cx.resources.tim.lock(|tim| tim.wait());
    }
}
