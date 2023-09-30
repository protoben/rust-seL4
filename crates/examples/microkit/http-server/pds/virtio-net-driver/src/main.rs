#![no_std]
#![no_main]
#![feature(never_type)]
#![feature(let_chains)]

use core::ptr::NonNull;

use virtio_drivers::{
    device::net::*,
    transport::{
        mmio::{MmioTransport, VirtIOHeader},
        DeviceType, Transport,
    },
};

use sel4_externally_shared::ExternallySharedRef;
use sel4_microkit::{memory_region_symbol, protection_domain, var, Channel};
use sel4_shared_ring_buffer::{RingBuffer, RingBuffers};
use sel4_hal_adapters::smoltcp::phy::{PhyDeviceHandler, IrqAck};

use microkit_http_server_example_virtio_hal_impl::HalImpl;
use microkit_http_server_example_smoltcp_phy_impl::{Device as PhyDeviceImpl, NET_QUEUE_SIZE, NET_BUFFER_LEN};

const DEVICE: Channel = Channel::new(0);
const CLIENT: Channel = Channel::new(1);

#[protection_domain(
    heap_size = 512 * 1024,
)]
fn init() -> PhyDeviceHandler<PhyDeviceImpl> {
    HalImpl::init(
        *var!(virtio_net_driver_dma_size: usize = 0),
        *var!(virtio_net_driver_dma_vaddr: usize = 0),
        *var!(virtio_net_driver_dma_paddr: usize = 0),
    );

    let mut dev = PhyDeviceImpl::new({
        let header = NonNull::new(
            (*var!(virtio_net_mmio_vaddr: usize = 0) + *var!(virtio_net_mmio_offset: usize = 0))
                as *mut VirtIOHeader,
        )
        .unwrap();
        let transport = unsafe { MmioTransport::new(header) }.unwrap();
        assert_eq!(transport.device_type(), DeviceType::Network);
        VirtIONet::<HalImpl, MmioTransport, NET_QUEUE_SIZE>::new(transport, NET_BUFFER_LEN).unwrap()
    });

    let client_region = unsafe {
        ExternallySharedRef::<'static, _>::new(
            memory_region_symbol!(virtio_net_client_dma_vaddr: *mut [u8], n = *var!(virtio_net_client_dma_size: usize = 0)),
        )
    };

    let client_client_dma_region_paddr = *var!(virtio_net_client_dma_paddr: usize = 0);

    let rx_ring_buffers = unsafe {
        RingBuffers::<'_, fn() -> Result<(), !>>::new(
            RingBuffer::from_ptr(memory_region_symbol!(virtio_net_rx_free: *mut _)),
            RingBuffer::from_ptr(memory_region_symbol!(virtio_net_rx_used: *mut _)),
            notify_client,
            true,
        )
    };

    let tx_ring_buffers = unsafe {
        RingBuffers::<'_, fn() -> Result<(), !>>::new(
            RingBuffer::from_ptr(memory_region_symbol!(virtio_net_tx_free: *mut _)),
            RingBuffer::from_ptr(memory_region_symbol!(virtio_net_tx_used: *mut _)),
            notify_client,
            true,
        )
    };

    dev.irq_ack();
    DEVICE.irq_ack().unwrap();

    PhyDeviceHandler::new(
        dev,
        client_region,
        client_client_dma_region_paddr,
        rx_ring_buffers,
        tx_ring_buffers,
        CLIENT,
        DEVICE,
    )
}

fn notify_client() -> Result<(), !> {
    CLIENT.notify();
    Ok::<_, !>(())
}
