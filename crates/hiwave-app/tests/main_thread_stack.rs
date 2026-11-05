//! The hiwave binary asks the linker for a 16 MiB main-thread stack
//! (build.rs), because page script runs on the AppKit main thread.

#[cfg(target_os = "macos")]
#[test]
fn hiwave_binary_has_a_16_mib_main_thread_stack() {
    let out = std::process::Command::new("otool")
        .args(["-l", env!("CARGO_BIN_EXE_hiwave")])
        .output()
        .expect("run otool");
    assert!(out.status.success(), "otool failed");
    let text = String::from_utf8_lossy(&out.stdout);
    // LC_MAIN's `stacksize` line; 0 means the 8 MiB OS default.
    let stacksize = text
        .lines()
        .skip_while(|l| !l.contains("cmd LC_MAIN"))
        .find_map(|l| l.trim().strip_prefix("stacksize "))
        .expect("LC_MAIN stacksize in otool -l output");
    assert_eq!(stacksize.trim(), "16777216");
}

#[cfg(target_os = "linux")]
#[test]
fn hiwave_binary_has_a_16_mib_main_thread_stack() {
    let out = std::process::Command::new("readelf")
        .args(["-lW", env!("CARGO_BIN_EXE_hiwave")])
        .output()
        .expect("run readelf");
    assert!(out.status.success(), "readelf failed");
    let text = String::from_utf8_lossy(&out.stdout);
    let stack_line = text
        .lines()
        .find(|l| l.contains("GNU_STACK"))
        .expect("GNU_STACK segment in readelf -lW output");
    let parts: Vec<&str> = stack_line.split_whitespace().collect();
    let memsz = parts.get(5).expect("memsz column in GNU_STACK line");
    assert_eq!(*memsz, "0x1000000", "expected 16 MiB stack (0x1000000), found {memsz}");

    // Runtime probe check: verifies RLIMIT_STACK re-exec took effect and runtime stack is 16 MiB
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_hiwave"))
        .arg("--check-stack")
        .output()
        .expect("run hiwave --check-stack");
    assert!(out.status.success(), "hiwave --check-stack failed: status={:?} stderr={}", out.status, String::from_utf8_lossy(&out.stderr));
    let sz_str = String::from_utf8_lossy(&out.stdout);
    let sz: usize = sz_str.trim().parse().expect("parsed stack size");
    assert!(sz >= 15 * 1024 * 1024, "expected at least 15 MiB runtime stack, found {sz} bytes");
}
