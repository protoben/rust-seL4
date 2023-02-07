#![no_std]
#![feature(core_intrinsics)]
#![feature(lang_items)]

extern crate sel4_runtime_simple_entry;

#[cfg(feature = "global-allocator")]
extern crate sel4_runtime_simple_static_heap;

use core::fmt;

use sel4_runtime_simple_termination::Termination;

pub use sel4_minimal_root_task_runtime_macros::main;

#[cfg(feature = "global-allocator")]
pub use sel4_runtime_simple_static_heap::set_mutex_notification as set_heap_mutex_notification;

#[macro_export]
macro_rules! declare_main {
    ($main:path) => {
        #[no_mangle]
        pub unsafe extern "C" fn __rust_entry(
            bootinfo: *const $crate::_private::seL4_BootInfo,
        ) -> ! {
            $crate::_private::run_main($main, bootinfo)
        }
    };
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn run_main<T>(
    f: impl Fn(&sel4::BootInfo) -> T,
    bootinfo: *const sel4::sys::seL4_BootInfo,
) -> !
where
    T: Termination,
    T::Error: fmt::Debug,
{
    let bootinfo = sel4::BootInfo::from_ptr(bootinfo);

    #[cfg(feature = "state")]
    sel4::set_ipc_buffer(bootinfo.ipc_buffer());

    let err = f(&bootinfo).report();

    sel4::debug_println!("Terminated with error: {:?}", err);
    abort()
}

#[cfg(panic = "unwind")]
compile_error!("");

#[cfg(feature = "panic-handler")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    sel4::debug_println!("{}", info);
    abort()
}

fn abort() -> ! {
    sel4::debug_println!("(aborting)");
    core::intrinsics::abort()
}

#[doc(hidden)]
pub mod _private {
    pub use crate::run_main;
    pub use sel4::sys::seL4_BootInfo;
}
