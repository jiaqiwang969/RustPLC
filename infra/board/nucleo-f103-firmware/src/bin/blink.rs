#![no_std]
#![no_main]

use cortex_m_rt::entry;
use nb::block;
use panic_halt as _;
use stm32f1xx_hal::{
    pac,
    prelude::*,
    serial::{Config, Serial},
};

fn uart_write(tx: &mut stm32f1xx_hal::serial::Tx<pac::USART2>, s: &str) {
    for b in s.as_bytes() {
        let _ = block!(tx.write(*b));
    }
}

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();

    // NUCLEO-F103RB: HSE is provided by the debugger MCO pin at 8 MHz.
    let clocks = rcc.cfgr.use_hse(8.MHz()).sysclk(72.MHz()).freeze(&mut flash.acr);

    let mut afio = dp.AFIO.constrain();
    let mut gpioa = dp.GPIOA.split();
    let mut gpioc = dp.GPIOC.split();

    // LD2 on NUCLEO-F103RB is typically PA5.
    let mut led_pa5 = gpioa.pa5.into_push_pull_output(&mut gpioa.crl);
    // Many F103 clone boards expose LED on PC13 (active-low on some boards).
    let mut led_pc13 = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

    let tx_pin = gpioa.pa2.into_alternate_push_pull(&mut gpioa.crl);
    let rx_pin = gpioa.pa3;

    let serial = Serial::new(
        dp.USART2,
        (tx_pin, rx_pin),
        &mut afio.mapr,
        Config::default().baudrate(115_200.bps()),
        &clocks,
    );
    let (mut tx, _rx) = serial.split();

    let mut delay = cp.SYST.delay(&clocks);
    let mut on = false;
    let mut tick: u32 = 0;

    uart_write(&mut tx, "blink firmware started (PA5/PC13)\r\n");

    loop {
        on = !on;
        tick = tick.wrapping_add(1);

        if on {
            let _ = led_pa5.set_high();
            let _ = led_pc13.set_low();
            uart_write(&mut tx, "blink ON ");
        } else {
            let _ = led_pa5.set_low();
            let _ = led_pc13.set_high();
            uart_write(&mut tx, "blink OFF ");
        }

        // Minimal decimal print without allocation.
        let mut buf = [0u8; 12];
        let mut i = buf.len();
        let mut n = tick;
        if n == 0 {
            i -= 1;
            buf[i] = b'0';
        } else {
            while n > 0 {
                i -= 1;
                buf[i] = b'0' + (n % 10) as u8;
                n /= 10;
            }
        }
        for b in &buf[i..] {
            let _ = block!(tx.write(*b));
        }
        uart_write(&mut tx, "\r\n");

        delay.delay_ms(250_u16);
    }
}
