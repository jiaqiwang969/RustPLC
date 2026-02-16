#![no_std]
#![no_main]

use cortex_m_rt::entry;
use nb::Error as NbError;
use panic_halt as _;
use stm32f1xx_hal::{
    pac,
    prelude::*,
    serial::{Config, Serial},
};

const UNIT_ID: u8 = 1;
const MAX_COILS: usize = 32;
const MAX_DI: usize = 32;
const RX_CAP: usize = 256;
const TX_CAP: usize = 256;

fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for b in data {
        crc ^= *b as u16;
        for _ in 0..8 {
            if (crc & 0x0001) != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

fn append_crc(frame: &mut [u8], len: usize) -> usize {
    let crc = crc16_modbus(&frame[..len]);
    frame[len] = (crc & 0xFF) as u8;
    frame[len + 1] = (crc >> 8) as u8;
    len + 2
}

fn check_crc(frame: &[u8]) -> bool {
    if frame.len() < 4 {
        return false;
    }
    let body_len = frame.len() - 2;
    let crc = crc16_modbus(&frame[..body_len]);
    frame[body_len] == (crc & 0xFF) as u8 && frame[body_len + 1] == (crc >> 8) as u8
}

fn expected_frame_len(buf: &[u8]) -> Option<usize> {
    if buf.len() < 2 {
        return None;
    }
    match buf[1] {
        0x01 | 0x02 | 0x05 => Some(8),
        0x0F => {
            if buf.len() < 7 {
                None
            } else {
                Some(9 + (buf[6] as usize))
            }
        }
        _ => Some(8),
    }
}

fn set_led_state(
    coil0: bool,
    led_pc13: &mut stm32f1xx_hal::gpio::gpioc::PC13<
        stm32f1xx_hal::gpio::Output<stm32f1xx_hal::gpio::PushPull>,
    >,
    led_pa5: &mut stm32f1xx_hal::gpio::gpioa::PA5<
        stm32f1xx_hal::gpio::Output<stm32f1xx_hal::gpio::PushPull>,
    >,
) {
    if coil0 {
        // PA5 high and PC13 low gives a visible indication across common F103 boards.
        let _ = led_pa5.set_high();
        let _ = led_pc13.set_low();
    } else {
        let _ = led_pa5.set_low();
        let _ = led_pc13.set_high();
    }
}

fn shift_left(buf: &mut [u8], len: &mut usize, n: usize) {
    if n >= *len {
        *len = 0;
        return;
    }
    let remain = *len - n;
    buf.copy_within(n..*len, 0);
    *len = remain;
}

fn set_di_from_coils(coils: &[bool; MAX_COILS], di: &mut [bool; MAX_DI]) {
    // Keep the default mapping simple for RustPLC validation.
    // DI0 mirrors coil0, DI1 mirrors coil1.
    di[0] = coils[0];
    di[1] = coils[1];
    // Additional helper bits for quick manual checks.
    di[2] = !coils[0];
    di[3] = !coils[1];
}

fn write_exception(tx: &mut [u8], unit: u8, function: u8, code: u8) -> usize {
    tx[0] = unit;
    tx[1] = function | 0x80;
    tx[2] = code;
    append_crc(tx, 3)
}

fn write_bit_response(tx: &mut [u8], unit: u8, function: u8, bits: &[bool], start: u16, qty: u16) -> usize {
    let qty_usize = qty as usize;
    let byte_count = (qty_usize + 7) / 8;

    tx[0] = unit;
    tx[1] = function;
    tx[2] = byte_count as u8;
    for i in 0..byte_count {
        tx[3 + i] = 0;
    }

    for i in 0..qty_usize {
        let bit = bits[start as usize + i];
        if bit {
            tx[3 + (i / 8)] |= 1u8 << (i % 8);
        }
    }

    append_crc(tx, 3 + byte_count)
}

fn handle_request(
    req: &[u8],
    coils: &mut [bool; MAX_COILS],
    di: &mut [bool; MAX_DI],
    tx: &mut [u8; TX_CAP],
) -> usize {
    let unit = req[0];
    let function = req[1];

    if unit != UNIT_ID && unit != 0 {
        return 0;
    }

    match function {
        // Read coils.
        0x01 => {
            let start = u16::from_be_bytes([req[2], req[3]]);
            let qty = u16::from_be_bytes([req[4], req[5]]);
            if qty == 0 || qty > 2000 {
                return write_exception(tx, unit, function, 0x03);
            }
            if (start as usize) + (qty as usize) > MAX_COILS {
                return write_exception(tx, unit, function, 0x02);
            }
            write_bit_response(tx, unit, function, coils, start, qty)
        }
        // Read discrete inputs.
        0x02 => {
            let start = u16::from_be_bytes([req[2], req[3]]);
            let qty = u16::from_be_bytes([req[4], req[5]]);
            if qty == 0 || qty > 2000 {
                return write_exception(tx, unit, function, 0x03);
            }
            if (start as usize) + (qty as usize) > MAX_DI {
                return write_exception(tx, unit, function, 0x02);
            }
            write_bit_response(tx, unit, function, di, start, qty)
        }
        // Write single coil.
        0x05 => {
            let addr = u16::from_be_bytes([req[2], req[3]]) as usize;
            let val = u16::from_be_bytes([req[4], req[5]]);
            if addr >= MAX_COILS {
                return write_exception(tx, unit, function, 0x02);
            }
            let state = match val {
                0xFF00 => true,
                0x0000 => false,
                _ => return write_exception(tx, unit, function, 0x03),
            };
            coils[addr] = state;
            set_di_from_coils(coils, di);

            // Echo request as response (standard FC05 behavior).
            tx[..6].copy_from_slice(&req[..6]);
            append_crc(tx, 6)
        }
        // Write multiple coils.
        0x0F => {
            let start = u16::from_be_bytes([req[2], req[3]]) as usize;
            let qty = u16::from_be_bytes([req[4], req[5]]) as usize;
            let byte_count = req[6] as usize;
            if qty == 0 || qty > 1968 {
                return write_exception(tx, unit, function, 0x03);
            }
            if start + qty > MAX_COILS {
                return write_exception(tx, unit, function, 0x02);
            }
            if byte_count != (qty + 7) / 8 {
                return write_exception(tx, unit, function, 0x03);
            }
            for i in 0..qty {
                let byte = req[7 + (i / 8)];
                coils[start + i] = ((byte >> (i % 8)) & 0x01) != 0;
            }
            set_di_from_coils(coils, di);

            // Response: unit, function, start, qty.
            tx[0] = unit;
            tx[1] = function;
            tx[2] = req[2];
            tx[3] = req[3];
            tx[4] = req[4];
            tx[5] = req[5];
            append_crc(tx, 6)
        }
        _ => write_exception(tx, unit, function, 0x01),
    }
}

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();
    let clocks = rcc.cfgr.use_hse(8.MHz()).sysclk(72.MHz()).freeze(&mut flash.acr);

    let mut afio = dp.AFIO.constrain();
    let mut gpioa = dp.GPIOA.split();
    let mut gpioc = dp.GPIOC.split();

    // See set_led_state(): PA5 and PC13 are driven in opposite levels.
    let mut led_pa5 = gpioa.pa5.into_push_pull_output(&mut gpioa.crl);
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
    let (mut tx_uart, mut rx_uart) = serial.split();

    let mut coils = [false; MAX_COILS];
    let mut di = [false; MAX_DI];
    let mut rx_buf = [0u8; RX_CAP];
    let mut rx_len: usize = 0;
    let mut tx_buf = [0u8; TX_CAP];

    set_di_from_coils(&coils, &mut di);
    set_led_state(coils[0], &mut led_pc13, &mut led_pa5);

    loop {
        match rx_uart.read() {
            Ok(b) => {
                if rx_len < RX_CAP {
                    rx_buf[rx_len] = b;
                    rx_len += 1;
                } else {
                    // Drop oversized garbage and resync quickly.
                    rx_len = 0;
                }
            }
            Err(NbError::WouldBlock) => {}
            Err(_) => {}
        }

        while rx_len >= 4 {
            let frame_len = match expected_frame_len(&rx_buf[..rx_len]) {
                Some(n) => n,
                None => break,
            };
            if rx_len < frame_len {
                break;
            }

            let frame = &rx_buf[..frame_len];
            if !check_crc(frame) {
                shift_left(&mut rx_buf, &mut rx_len, 1);
                continue;
            }

            let resp_len = handle_request(frame, &mut coils, &mut di, &mut tx_buf);
            if resp_len > 0 && frame[0] != 0 {
                for b in &tx_buf[..resp_len] {
                    let _ = nb::block!(tx_uart.write(*b));
                }
            }

            set_led_state(coils[0], &mut led_pc13, &mut led_pa5);
            shift_left(&mut rx_buf, &mut rx_len, frame_len);
        }
    }
}
