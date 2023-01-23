use std::path::Path;

#[rustfmt::skip]
const BLOCKLIST: &[&'static str] = &[
    "CONFIG_.*",
    "LIBSEL4_MCS_REPLY",
    "__sel4_ipc_buffer",

    ".*_t",

    // generated enums
    "seL4_Syscall_ID",
    ".*invocation_label",

    // depricated
    "seL4_AsyncEndpointObject",
    "seL4_PageFaultIpcRegisters.*",

    // static checks
    "__type_.*_size_incorrect",
];

const HEADER_CONTENTS: &'static str = r#"
    // HACK for upstream Docker container toolchain issue (very temporary)
    #define __thread

    #include <sel4/sel4.h>
    #include <sel4/arch/mapping.h>
    #include <sel4/sel4_arch/mapping.h>
"#;

pub fn generate_rust(
    libsel4_include_dirs: impl Iterator<Item = impl AsRef<Path>>,
    extra_blocklist: &[String],
) -> bindgen::Bindings {
    let mut builder = bindgen::Builder::default()
        .header_contents("wrapper.h", HEADER_CONTENTS)
        .detect_include_paths(false)
        .clang_args(libsel4_include_dirs.map(|d| format!("-I{}", d.as_ref().display())))
        .ignore_functions();

    for item in BLOCKLIST.iter() {
        builder = builder.blocklist_item(item);
    }

    for item in extra_blocklist.iter() {
        builder = builder.blocklist_item(item);
    }

    // HACK for risc64imac (already handled in upstream bindgen for riscv64gc)
    if sel4_config::sel4_cfg_bool!(ARCH_RISCV64) {
        builder = builder.clang_arg("--target=riscv64-unknown-linux-gnu");
    }

    builder
        .rust_target(bindgen::RustTarget::Nightly)
        .constified_enum_module(".*")
        .derive_eq(true)
        .derive_default(true)
        .generate_comments(false)
        .use_core()
        .ctypes_prefix("::core::ffi")
        .rustfmt_bindings(true)
        .generate()
        .unwrap()
}
