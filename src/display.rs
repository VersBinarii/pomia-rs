use crate::clock::{RtcClock, Time};
use embedded_graphics::{
    fonts::{Font12x16, Font8x16, Text},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Line, Rectangle},
    style::{MonoTextStyleBuilder, PrimitiveStyle},
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

#[derive(Copy, Clone)]
pub struct ClockState {
    edit: u8,
    time: Time,
}

impl ClockState {
    pub fn with_time(time: Time) -> Self {
        Self { edit: 0, time }
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
        Self {
            display,
            menu: [View::Measure, View::Clock(ClockState::with_time(0.into()))],
            pointer: 0,
            rerender: false,
        }
    }

    pub fn forward(&mut self) {
        match self.current_menu_item() {
            View::Clock(mut state) if state.editing() => {
                if state.edit & EDIT_H != 0 {
                    state.time.hours += 1;
                    if state.time.hours > 24 {
                        state.time.hours = 0;
                    }
                }
                if state.edit & EDIT_M != 0 {
                    state.time.minutes += 1;
                    if state.time.minutes > 59 {
                        state.time.minutes = 0;
                    }
                }
                if state.edit & EDIT_S != 0 {
                    state.time.seconds += 1;
                    if state.time.seconds > 59 {
                        state.time.seconds = 0;
                    }
                }
                core::mem::swap(&mut self.menu[1], &mut View::Clock(state));
            }
            _ => {
                self.pointer += 1;
                if self.pointer > 1 {
                    self.pointer = 0;
                }
            }
        }

        self.rerender = true;
    }

    pub fn backward(&mut self) {
        match self.current_menu_item() {
            View::Clock(mut state) if state.editing() => {
                if state.edit & EDIT_H != 0 {
                    state.time.hours -= 1;
                    if let None = state.time.hours.checked_sub(1) {
                        state.time.hours = 23;
                    }
                }
                if state.edit & EDIT_M != 0 {
                    state.time.minutes -= 1;
                    if let None = state.time.minutes.checked_sub(1) {
                        state.time.minutes = 59;
                    }
                }
                if state.edit & EDIT_S != 0 {
                    state.time.seconds -= 1;
                    if let None = state.time.seconds.checked_sub(1) {
                        state.time.seconds = 59;
                    }
                }
                core::mem::swap(&mut self.menu[1], &mut View::Clock(state));
            }
            _ => {
                self.pointer -= 1;
                if self.pointer < 0 {
                    self.pointer = 1;
                }
            }
        }

        self.rerender = true;
    }

    fn current_menu_item(&self) -> View {
        self.menu[self.pointer as usize]
    }

    pub fn edit_clock(&mut self, clock: &mut RtcClock) {
        match self.current_menu_item() {
            View::Clock(mut state) if state.editing() => {
                clock.set_time(&state.time);
                state.edit = 0;
                core::mem::swap(&mut self.menu[1], &mut View::Clock(state));
            }
            View::Clock(state) if !state.editing() => {
                let mut cs = ClockState::with_time(clock.get_time());
                cs.edit |= EDIT | EDIT_H;
                core::mem::swap(&mut self.menu[1], &mut View::Clock(cs));
            }
            _ => {}
        }

        self.rerender = true;
    }

    pub fn select(&mut self) {
        match self.current_menu_item() {
            View::Clock(mut state) if state.editing() => {
                let mut tmp = state.edit & 0x7;
                tmp >>= 1;
                if tmp == 0 {
                    tmp = 4;
                }
                state.edit &= !0x7;
                state.edit |= tmp;
                core::mem::swap(&mut self.menu[1], &mut View::Clock(state));
                self.rerender = true;
            }
            _ => {}
        }
    }

    pub fn print_header(&mut self) {
        if self.rerender {
            self.display.clear();
            self.rerender = false;
        }
        let text = match self.current_menu_item() {
            View::Measure => "Measurements",
            View::Clock(clock_state) if clock_state.editing() => "Clock (Edit)",
            View::Clock(_) => "Clock",
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
                "T: {} C\n\nH: {} %\n\nP: {} Pa",
                stats.0,
                stats.1,
                stats.2
            );
            self.display.print_text_lg(&text, 0, 30);
        }
    }

    pub fn print_clock(&mut self, clock: &RtcClock) {
        if let View::Clock(state) = self.current_menu_item() {
            if self.rerender {
                self.display.clear();
                self.rerender = false;
            }

            let y_position = 46;
            let mut x_position = 0;
            let mut text: String<U16> = String::new();
            match state.editing() {
                false => {
                    let _ = uwrite!(text, "{}", clock.get_time());
                }
                true => {
                    //display the edit pointer
                    let _ = uwrite!(text, "{}", state.time);

                    if state.edit & EDIT_H != 0 {
                        //underline hours
                        x_position = 10;
                    }
                    if state.edit & EDIT_M != 0 {
                        //underline minutes
                        x_position = 45;
                    }
                    if state.edit & EDIT_S != 0 {
                        //underline seconds
                        x_position = 80;
                    }
                }
            }
            if x_position != 0 {
                self.display.print_pointer(
                    Point::new(x_position, y_position),
                    Point::new(x_position + 20, y_position),
                );
            }
            self.display.print_text_lg(&text, 10, 30);
        }
    }

    pub fn print_error(&mut self, error: impl uDebug) {
        let mut text: String<U16> = String::new();
        let _ = uwrite!(text, "{:?}", error);
        self.display.print_text_sm(&text, 10, 30);
    }
}

pub struct Display {
    display: DISP,
}

impl Display {
    pub fn new(display: DISP) -> Self {
        Self { display }
    }

    pub fn print_text_sm(&mut self, text: &str, x: i32, y: i32) {
        let style = MonoTextStyleBuilder::new(Font8x16)
            .text_color(Rgb565::RED)
            .background_color(Rgb565::BLACK)
            .build();
        Text::new(text, Point::new(x, y))
            .into_styled(style)
            .draw(&mut self.display)
            .unwrap();
    }

    pub fn print_text_lg(&mut self, text: &str, x: i32, y: i32) {
        let style = MonoTextStyleBuilder::new(Font12x16)
            .text_color(Rgb565::RED)
            .background_color(Rgb565::BLACK)
            .build();
        Text::new(text, Point::new(x, y))
            .into_styled(style)
            .draw(&mut self.display)
            .unwrap();
    }

    pub fn render_tab_header(&mut self, text: &str) {
        let thick_stroke = PrimitiveStyle::with_stroke(Rgb565::MAGENTA, 3);

        Rectangle::new(Point::zero(), Size::new(128, 20))
            .into_styled(thick_stroke)
            .draw(&mut self.display)
            .unwrap();
        self.print_text_sm(text, (128 - (text.len() * 8) as i32) / 2, 3);
    }

    pub fn print_pointer(&mut self, start: Point, end: Point) {
        Line::new(start, end)
            .into_styled(PrimitiveStyle::with_stroke(Rgb565::RED, 1))
            .draw(&mut self.display)
            .unwrap();
    }

    pub fn clear(&mut self) {
        self.display.clear(Rgb565::BLACK).unwrap();
    }
}
