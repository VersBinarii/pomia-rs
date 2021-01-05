use heapless::{consts::*, String};
use stm32f1xx_hal::rtc::Rtc;
use ufmt::{uDisplay, uWrite, uwrite, Formatter};

#[derive(Copy, Clone)]
pub struct Time {
    pub hours: u8,
    pub minutes: u8,
    pub seconds: u8,
}

impl uDisplay for Time {
    fn fmt<W: uWrite + ?Sized>(&self, f: &mut Formatter<'_, W>) -> Result<(), W::Error> {
        let _ = f.write_str(&zero_pad(self.hours));
        let _ = f.write_str(":");
        let _ = f.write_str(&zero_pad(self.minutes));
        let _ = f.write_str(":");
        f.write_str(&zero_pad(self.seconds))
    }
}

fn zero_pad(num: u8) -> String<U2> {
    let mut num_str = String::new();
    if num < 10 {
        num_str.clear();
        uwrite!(num_str, "0{}", num).unwrap();
    } else {
        uwrite!(num_str, "{}", num).unwrap();
    }

    num_str
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

impl core::convert::From<&Time> for u32 {
    fn from(val: &Time) -> Self {
        val.hours as u32 * 3600 + val.minutes as u32 * 60 + val.seconds as u32
    }
}
pub struct Clock {
    rtc: Rtc,
}

impl Clock {
    pub fn new(rtc: Rtc) -> Self {
        Self { rtc }
    }

    pub fn get_time(&self) -> Time {
        let time_val = self.rtc.current_time();
        time_val.into()
    }

    pub fn set_time(&mut self, time: &Time) {
        self.rtc.set_time(time.into())
    }
}
