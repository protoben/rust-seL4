use spin::Mutex;

use crate::drivers::bcm2835_aux_uart::Bcm2835AuxUartDevice;

const BASE_ADDR: usize = 0xfe21_5000;

static DEVICE: Mutex<Bcm2835AuxUartDevice> = Mutex::new(get_device());

const fn get_device() -> Bcm2835AuxUartDevice {
    unsafe { Bcm2835AuxUartDevice::new(BASE_ADDR) }
}

pub(crate) fn init() {
    DEVICE.lock().init();
}

pub(crate) fn put_char(c: u8) {
    DEVICE.lock().put_char(c);
}

pub(crate) fn put_char_without_synchronization(c: u8) {
    get_device().put_char(c);
}