# pomia-rs
Were testing here how well Rust works for an embedded project

So far we have up and running:
* [RTIC][1]
* Timer3 interrupt
* PWM used for generating music
* SPI for driving 128x160 LCD display
* Some basic graphics based on [embedded_graphics][2]
* Basic UI allowing changing views and basic edit mode.
* I2C based temperature/humidity/pressure sensor BME280
* EXTI interrupt based button handling
* RTC based clock 

# Youtube video
There is a bunch of videos on youtube showing progress in implementing the above functionality. You can find it [here][3]

[1]: https://github.com/rtic-rs/cortex-m-rtic
[2]: https://github.com/embedded-graphics/embedded-graphics
[3]: https://www.youtube.com/watch?v=Meqhiogdp1o
