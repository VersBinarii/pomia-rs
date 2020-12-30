use core::fmt::Write;
use embedded_graphics::{
    fonts::{Font8x16, Text},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    style::{MonoTextStyle, MonoTextStyleBuilder, PrimitiveStyle},
};
use heapless::{consts::*, String};
use st7735_lcd::ST7735;
use stm32f1xx_hal::{
    gpio::{
        gpioa::{PA5, PA6, PA7},
        gpiob::{PB0, PB1},
        Alternate, Floating, Input, Output, PushPull,
    },
    pac::SPI1,
    rtc::Rtc,
    spi::{Spi, Spi1NoRemap},
};

type RESET = PB0<Output<PushPull>>;
type DC = PB1<Output<PushPull>>;
type SCK = PA5<Alternate<PushPull>>;
type MISO = PA6<Input<Floating>>;
type MOSI = PA7<Alternate<PushPull>>;
type DISP = ST7735<Spi<SPI1, Spi1NoRemap, (SCK, MISO, MOSI), u8>, DC, RESET>;

pub struct Time {
    hours: u8,
    minutes: u8,
    seconds: u8,
}

impl core::fmt::Display for Time {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:02}:{:02}:{:02}",
            self.hours, self.minutes, self.seconds
        )
    }
}

impl core::convert::From<u32> for Time {
    fn from(val: u32) -> Self {
        let hours = (val / 3600) as u8;
        let minutes = ((val % 3600) / 60) as u8;
        let seconds = ((val % 3600) % 60) as u8;

        Self {
            hours,
            minutes,
            seconds,
        }
    }
}

pub struct Watch {
    rtc: Rtc,
}

impl Watch {
    pub fn new(rtc: Rtc) -> Self {
        Self { rtc }
    }

    pub fn get_time(&self) -> Time {
        let time_val = self.rtc.current_time();
        time_val.into()
    }
}

#[derive(Copy, Clone)]
pub enum View {
    Measure,
    Watch,
}

pub struct Gui {
    display: Display,
    pointer: i8,
    menu: [View; 2],
    rerender: bool,
}
impl Gui {
    pub fn new(display: Display) -> Self {
        use View::*;
        Self {
            display,
            menu: [Measure, Watch],
            pointer: 0,
            rerender: false,
        }
    }

    pub fn forward(&mut self) {
        self.pointer += 1;
        if self.pointer > 1 {
            self.pointer = 0;
        }

        self.rerender = true;
    }

    pub fn backward(&mut self) {
        self.pointer -= 1;
        if self.pointer < 0 {
            self.pointer = 1;
        }

        self.rerender = true;
    }

    pub fn current_menu_item(&self) -> View {
        self.menu[self.pointer as usize]
    }

    pub fn print_header(&mut self) {
        if self.rerender {
            self.display.clear();
            self.rerender = false;
        }
        let text = match self.current_menu_item() {
            View::Measure => "Measurements",
            View::Watch => "Clock",
        };
        self.display.render_tab_header(&text);
    }

    pub fn print_measurements(
        &mut self,
        stats: (
            u8,  /* Temp */
            u8,  /* Hum */
            u32, /* Pressure */
        ),
    ) {
        if let View::Measure = self.current_menu_item() {
            if self.rerender {
                self.display.clear();
                self.rerender = false;
            }
            let mut text: String<U32> = String::new();

            let _ = write!(
                text,
                "T: {:.1} C\nH: {:.2} %\nP: {:.0} Pa",
                stats.0, stats.1, stats.2
            );
            self.display.print_text(&text, 0, 30);
        }
    }

    pub fn print_clock(&mut self, watch: &Watch) {
        if let View::Watch = self.current_menu_item() {
            if self.rerender {
                self.display.clear();
                self.rerender = false;
            }
            let mut text: String<U16> = String::new();
            let _ = write!(text, "{}", watch.get_time());
            self.display.print_text(&text, 10, 30);
        }
    }

    pub fn print_error(&mut self, error: impl core::fmt::Debug) {
        let mut text: String<U16> = String::new();
        let _ = write!(text, "{:?}", error);
        self.display.print_text(&text, 10, 30);
    }
}

pub struct Display {
    style: MonoTextStyle<Rgb565, Font8x16>,
    display: DISP,
}

impl Display {
    pub fn new(display: DISP) -> Self {
        let style = MonoTextStyleBuilder::new(Font8x16)
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

    pub fn render_tab_header(&mut self, text: &str) {
        let thick_stroke = PrimitiveStyle::with_stroke(Rgb565::MAGENTA, 3);

        Rectangle::new(Point::zero(), Size::new(128, 20))
            .into_styled(thick_stroke)
            .draw(&mut self.display)
            .unwrap();
        self.print_text(text, (128 - (text.len() * 8) as i32) / 2, 3);
    }

    pub fn clear(&mut self) {
        self.display.clear(Rgb565::BLACK);
    }
}
