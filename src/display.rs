use embedded_graphics::{
    fonts::{Font8x16, Text},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    style::{MonoTextStyle, MonoTextStyleBuilder, PrimitiveStyle},
};
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

type RESET = PB0<Output<PushPull>>;
type DC = PB1<Output<PushPull>>;
type SCK = PA5<Alternate<PushPull>>;
type MISO = PA6<Input<Floating>>;
type MOSI = PA7<Alternate<PushPull>>;
type DISP = ST7735<Spi<SPI1, Spi1NoRemap, (SCK, MISO, MOSI), u8>, DC, RESET>;

#[derive(Copy, Clone)]
pub enum MenuItem {
    Temperature,
    Humidity,
    Pressure,
}

pub struct Menu {
    pointer: i32,
    menu: [MenuItem; 3],
    pub confirm: bool,
}
impl Menu {
    pub fn new() -> Self {
        use MenuItem::*;
        Self {
            menu: [Temperature, Humidity, Pressure],
            pointer: 0,
            confirm: false,
        }
    }

    pub fn forward(&mut self) {
        self.pointer += 1;
        if self.pointer > 2 {
            self.pointer = 0;
        }
    }

    pub fn backward(&mut self) {
        self.pointer -= 1;
        if self.pointer < 0 {
            self.pointer = 2;
        }
    }

    pub fn current_menu_item(&self) -> MenuItem {
        self.menu[self.pointer as usize]
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

    pub fn render_tab_header(&mut self, menu_item: MenuItem) {
        let thick_stroke = PrimitiveStyle::with_stroke(Rgb565::MAGENTA, 3);

        Rectangle::new(Point::zero(), Size::new(128, 20))
            .into_styled(thick_stroke)
            .draw(&mut self.display)
            .unwrap();
        let text = "               ";
        self.print_text(text, (128 - (text.len() * 8) as i32) / 2, 3);
        match menu_item {
            MenuItem::Temperature => {
                let text = "Temperature";
                self.print_text(text, (128 - (text.len() * 8) as i32) / 2, 3)
            }
            MenuItem::Humidity => {
                let text = "Humidity";
                self.print_text(text, (128 - (text.len() * 8) as i32) / 2, 3)
            }
            MenuItem::Pressure => {
                let text = "Pressure";
                self.print_text(text, (128 - (text.len() * 8) as i32) / 2, 3)
            }
        }
    }
}
