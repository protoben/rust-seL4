#![no_std]

//! A smoltcp::phy::Driver impl for VirtIONet.

extern crate alloc;
use alloc::rc::Rc;

use core::cell::RefCell;

use virtio_drivers::device::net::{self, RxBuffer};
use virtio_drivers::transport::mmio::MmioTransport;

use microkit_http_server_example_virtio_hal_impl::HalImpl;

use smoltcp::phy;
use smoltcp::time::Instant;

pub const NET_QUEUE_SIZE: usize = 16;
pub const NET_BUFFER_LEN: usize = 2048;

pub trait IrqAck {
    fn irq_ack(&mut self);
}

pub trait HasMac {
    fn mac_address(&self) -> [u8; 6];
}

pub type VirtIONet = net::VirtIONet<HalImpl, MmioTransport, {NET_QUEUE_SIZE}>;

pub struct RxToken {
    dev_inner: Rc<RefCell<VirtIONet>>,
    // NOTE This is an option so we can call mem::take on it for implementing
    // Drop. This is necessary because virtio's deallocation function moves
    // its argument, which we can't normally do in Drop::drop.
    buf: Option<RxBuffer>,
}

impl phy::RxToken for RxToken {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(mut self, f: F) -> R {
        // XXX: Why is this mut? We could avoid calling packet_mut by
        // creating a temporary vector, but this would add a copy.
        f(self.buf.as_mut().unwrap().packet_mut())
    }
}

impl Drop for RxToken {
    fn drop(&mut self) {
        let _ = self.dev_inner.borrow_mut().recycle_rx_buffer(self.buf.take().unwrap());
    }
}

pub struct TxToken {
    dev_inner: Rc<RefCell<VirtIONet>>,
}

impl phy::TxToken for TxToken {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
        let mut dev = self.dev_inner.borrow_mut();

        let mut buf = dev.new_tx_buffer(len);
        let res = f(buf.packet_mut());
        // XXX How can we avoid panicking here? Appears to fail only if the
        // queue is full.
        dev.send(buf).expect("Failed to send buffer");

        res
    }
}

pub struct Device {
    dev_inner: Rc<RefCell<VirtIONet>>,
}

impl Device {
    pub fn new(dev: VirtIONet) -> Self {
        Self {
            dev_inner: Rc::new(RefCell::new(dev)),
        }
    }
}

impl phy::Device for Device {
    type RxToken<'a> = RxToken;
    type TxToken<'a> = TxToken;

    fn receive(
        &mut self,
        _timestamp: Instant,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let mut dev = self.dev_inner.borrow_mut();

        if !dev.can_recv() || !dev.can_send() {
            return None;
        }

        let rx_buf = dev.receive().ok()?;

        Some((
            RxToken {
                dev_inner: self.dev_inner.clone(),
                buf: Some(rx_buf),
            },
            TxToken {
                dev_inner: self.dev_inner.clone(),
            },
        ))
    }

    fn transmit(
        &mut self,
        _timestamp: Instant,
    ) -> Option<Self::TxToken<'_>> {
        if !self.dev_inner.borrow().can_send() {
            return None;
        }

        Some(TxToken {
            dev_inner: self.dev_inner.clone(),
        })
    }

    fn capabilities(&self) -> phy::DeviceCapabilities {
        // XXX What are these for virtio?
        phy::DeviceCapabilities::default()
    }
}

impl IrqAck for Device {
    fn irq_ack(&mut self) {
        self.dev_inner.borrow_mut().ack_interrupt();
    }
}

impl HasMac for Device {
    fn mac_address(&self) -> [u8; 6] {
        self.dev_inner.borrow().mac_address()
    }
}
