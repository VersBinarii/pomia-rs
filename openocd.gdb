file target/thumbv7m-none-eabi/release/pomia-rs
target remote :3333

set print asm-demangle on
set print pretty on

load

#break main

continue
