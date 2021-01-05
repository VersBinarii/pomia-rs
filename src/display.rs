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
    spi::{Spi, Spi1NoRemap},
};
use ufmt::{uDebug, uwrite};

type RESET = PB0<Output<PushPull>>;
type DC = PB1<Output<PushPull>>;
type SCK = PA5<Alternate<PushPull>>;
type MISO = PA6<Input<Floating>>;
type MOSI = PA7<Alternate<PushPull>>;
type DISP = ST7735<Spi<SPI1, Spi1NoRemap, (SCK, MISO, MOSI), u8>, DC, RESET>;

const EDIT: u8 = 8;
const EDIT_H: u8 = 4;
const EDIT_M: u8 = 2;
const EDIT_S: u8 = 1;

#[derive(Copy, Clone, Default)]
pub struct ClockState {
    edit: u8,
    time: Time,
}

impl ClockState {
    pub fn new(clock: &Clock) -> Self {
        Self {
            edit: 0,
            time: clock.get_time(),
        }
    }

    pub fn editing(&self) -> bool {
        self.edit & EDIT != 0
    }
}

#[derive(Copy, Clone)]
pub enum View {
    Measure,
    Clock(ClockState),
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
            menu: [Measure, Clock],
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
            View::Clock => "Clock",
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

            let _ = uwrite!(
                text,
                "T: {} C\nH: {} %\nP: {} Pa",
                stats.0,
                stats.1,
                stats.2
            );
            self.display.print_text(&text, 0, 30);
        }
    }

    pub fn print_clock(&mut self, clock: &Clock) {
        if let View::Clock = self.current_menu_item() {
            if self.rerender {
                self.display.clear();
                self.rerender = false;
            }
            let mut text: String<U16> = String::new();
            let _ = uwrite!(text, "{}", clock.get_time());
            self.display.print_text(&text, 10, 30);
        }
    }

    pub fn print_error(&mut self, error: impl uDebug) {
        let mut text: String<U16> = String::new();
        let _ = uwrite!(text, "{:?}", error);
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
        self.display.clear(Rgb565::BLACK).unwrap();
    }
}
