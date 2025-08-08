use injectorpp::interface::injector::*;
use std::thread;

// Make the entry block >12 bytes on AArch64 so preflight doesn't panic
#[inline(never)]
#[no_mangle] // keep a concrete symbol; also helps prevent over-aggressive opts/inlining
pub fn foo() -> i32 {
    // Force a small stack frame and real memory traffic.
    let mut a: i32 = 1;
    let mut b: i32 = 2;

    unsafe {
        // Volatile writes prevent the compiler from collapsing this to a constant.
        core::ptr::write_volatile(&mut a, a.wrapping_add(b)); // a = 3
        core::ptr::write_volatile(&mut b, 3);                 // b = 3
    }

    let c = a + b; // 3 + 3 = 6

    // Volatile read keeps the load and return path as code, not folded to imm.
    unsafe { core::ptr::read_volatile(&c as *const i32) }
}


#[test]
fn test_multi_thread_function_call() {
    let handle = thread::spawn(move || {
        for _ in 0..1000 {
            let _guard = InjectorPP::prevent();

            assert_eq!(foo(), 6);
        }
    });

    for _ in 0..10 {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(fn (foo)() -> i32))
            .will_execute_raw(injectorpp::closure!(|| { 9 }, fn() -> i32));

        assert_eq!(foo(), 9);
    }

    handle.join().unwrap();
}

#[test]
fn test_original_function_call() {
    let _guard = InjectorPP::prevent();

    assert_eq!(foo(), 6);
}

#[test]
fn test_faked_function_call() {
    let mut injector = InjectorPP::new();
    injector
        .when_called(injectorpp::func!(fn (foo)() -> i32))
        .will_execute_raw(injectorpp::closure!(|| { 9 }, fn() -> i32));

    assert_eq!(foo(), 9);
}
