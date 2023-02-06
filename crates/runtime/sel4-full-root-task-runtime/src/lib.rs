#![no_std]

extern crate sel4_runtime_simple_entry;

#[cfg(feature = "global-allocator")]
extern crate sel4_runtime_simple_static_heap;

use core::ffi::c_char;
use core::ffi::c_void;
use core::fmt;

#[cfg(feature = "unwinding")]
mod unwinding;

use sel4_runtime_phdrs::EmbeddedProgramHeaders;

pub use sel4_full_root_task_runtime_macros::main;
pub use sel4_panicking::catch_unwind;
pub use sel4_panicking_env::{abort, debug_print, debug_println};
pub use sel4_runtime_simple_termination::Termination;

#[cfg(feature = "global-allocator")]
pub use sel4_runtime_simple_static_heap::set_mutex_notification as set_heap_mutex_notification;

#[no_mangle]
pub unsafe extern "C" fn __rust_entry(bootinfo: *const sel4::sys::seL4_BootInfo) -> ! {
    let cont_arg = bootinfo.cast::<c_void>().cast_mut();
    EmbeddedProgramHeaders::finder()
        .find_tls_image()
        .reserve_on_stack_and_continue(cont_fn, cont_arg)
}

pub unsafe extern "C" fn cont_fn(cont_arg: *mut c_void) -> ! {
    let bootinfo = cont_arg.cast_const().cast::<sel4::sys::seL4_BootInfo>();

    #[cfg(feature = "unwinding")]
    {
        crate::unwinding::init();
    }

    sel4::set_ipc_buffer(sel4::BootInfo::from_ptr(bootinfo).ipc_buffer());
    __sel4_for_simple_root_task_main(bootinfo);
    abort()
}

extern "C" {
    fn __sel4_for_simple_root_task_main(bootinfo: *const sel4::sys::seL4_BootInfo);
}

#[macro_export]
macro_rules! declare_main {
    ($main:path) => {
        #[no_mangle]
        pub extern "C" fn __sel4_for_simple_root_task_main(
            bootinfo: *const $crate::_private::seL4_BootInfo,
        ) {
            $crate::_private::run_main($main, bootinfo);
        }
    };
}

pub fn run_main<T>(f: impl Fn(&sel4::BootInfo) -> T, bootinfo: *const sel4::sys::seL4_BootInfo)
where
    T: Termination,
    T::Error: fmt::Debug,
{
    let _ = catch_unwind(|| {
        let bootinfo = unsafe { sel4::BootInfo::from_ptr(bootinfo) };
        let err = f(&bootinfo).report();
        sel4::debug_println!("Terminated with error: {:?}", err);
    });
}

#[no_mangle]
fn sel4_runtime_debug_put_char(c: c_char) {
    sel4::debug_put_char(c)
}

// For macros
#[doc(hidden)]
pub mod _private {
    pub use super::run_main;
    pub use sel4::sys::seL4_BootInfo;
}
