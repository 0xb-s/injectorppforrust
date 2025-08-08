#![cfg(target_arch = "aarch64")]

use crate::injector_core::arm64_codegenerator::*;
use crate::injector_core::common::*;
use crate::injector_core::patch_trait::*;
use crate::injector_core::utils::*;

pub(crate) struct PatchArm64;

impl PatchTrait for PatchArm64 {
    fn replace_function_with_other_function(
        src: FuncPtrInternal,
        target: FuncPtrInternal,
    ) -> PatchGuard {
        const PATCH_SIZE: usize = 12;
        const JIT_SIZE: usize = 20;

        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        unsafe {
            assert_min_patch_window_or_panic(&src, PATCH_SIZE);
        }

        let original_bytes = unsafe { read_bytes(src.as_ptr() as *mut u8, PATCH_SIZE) };
        let jit_memory = allocate_jit_memory(&src, JIT_SIZE);
        generate_will_execute_jit_code_abs(jit_memory, target.as_ptr());

        apply_branch_patch(src, jit_memory, JIT_SIZE, &original_bytes)
    }

    fn replace_function_return_boolean(src: FuncPtrInternal, value: bool) -> PatchGuard {
        const PATCH_SIZE: usize = 12;
        const JIT_SIZE: usize = 8;

        // NEW: Linux-only preflight guard.
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        unsafe {
            assert_min_patch_window_or_panic(&src, PATCH_SIZE);
        }

        let original_bytes = unsafe { read_bytes(src.as_ptr() as *mut u8, PATCH_SIZE) };
        let jit_memory = allocate_jit_memory(&src, JIT_SIZE);
        generate_will_return_boolean_jit_code(jit_memory, value);

        apply_branch_patch(src, jit_memory, JIT_SIZE, &original_bytes)
    }
}

/// Generates a 20-byte JIT code block that loads the absolute address of `target`
/// into X9 (MOVZ + 3×MOVK) and then branches to X9 (BR).
fn generate_will_execute_jit_code_abs(jit_ptr: *mut u8, target: *const ()) {
    let target_addr = target as usize as u64;

    // x9
    let register_name: [bool; 5] = u8_to_bits::<5>(9);

    // MOVZ x9, #imm0 (clears the rest)
    let movz = emit_movz_from_address(target_addr, 0, true, u8_to_bits::<2>(0), register_name);

    // MOVK x9, #imm1, LSL #16
    let movk1 = emit_movk_from_address(target_addr, 16, true, u8_to_bits::<2>(1), register_name);

    // MOVK x9, #imm2, LSL #32
    let movk2 = emit_movk_from_address(target_addr, 32, true, u8_to_bits::<2>(2), register_name);

    // MOVK x9, #imm3, LSL #48
    let movk3 = emit_movk_from_address(target_addr, 48, true, u8_to_bits::<2>(3), register_name);

    // BR x9
    let br = emit_br(register_name);

    let mut asm_code: Vec<u8> = Vec::new();
    append_instruction(&mut asm_code, bool_array_to_u32(movz));
    append_instruction(&mut asm_code, bool_array_to_u32(movk1));
    append_instruction(&mut asm_code, bool_array_to_u32(movk2));
    append_instruction(&mut asm_code, bool_array_to_u32(movk3));
    append_instruction(&mut asm_code, bool_array_to_u32(br));

    unsafe {
        inject_asm_code(&asm_code, jit_ptr);
    }
}

/// Generates an 8-byte JIT block that returns the specified boolean.
/// The code moves the immediate into w0 and then returns.
fn generate_will_return_boolean_jit_code(jit_ptr: *mut u8, value: bool) {
    let mut asm_code = [0u8; 8]; // 2 instructions = 2 * 4
    let mut cursor = 0;

    let mut value_bits = [false; 16];
    value_bits[0] = value;

    let movz = emit_movz(value_bits, true, u8_to_bits::<2>(0), u8_to_bits::<5>(0));
    let ret = emit_ret_x30();

    write_instruction(&mut asm_code, &mut cursor, bool_array_to_u32(movz));
    write_instruction(&mut asm_code, &mut cursor, bool_array_to_u32(ret));

    unsafe {
        inject_asm_code(&asm_code, jit_ptr);
    }
}

#[inline]
fn write_instruction(buf: &mut [u8], cursor: &mut usize, instruction: u32) {
    let bytes = instruction.to_le_bytes();
    buf[*cursor..*cursor + 4].copy_from_slice(&bytes);
    *cursor += 4;
}

fn append_instruction(asm_code: &mut Vec<u8>, instruction: u32) {
    asm_code.push((instruction & 0xFF) as u8);
    asm_code.push(((instruction >> 8) & 0xFF) as u8);
    asm_code.push(((instruction >> 16) & 0xFF) as u8);
    asm_code.push(((instruction >> 24) & 0xFF) as u8);
}

fn apply_branch_patch(
    src: FuncPtrInternal,
    jit_memory: *mut u8,
    jit_size: usize,
    original_bytes: &[u8],
) -> PatchGuard {
    const PATCH_SIZE: usize = 12;
    const NOP: u32 = 0xd503201f;

    let func_addr = src.as_ptr() as usize;
    let jit_addr = jit_memory as usize;

    let mut patch = [0u8; PATCH_SIZE];

    // macOS path: your existing long-jump helper.
    #[cfg(target_os = "macos")]
    {
        let instrs = maybe_emit_long_jump(func_addr, jit_addr);
        if instrs.len() == 1 {
            patch[0..4].copy_from_slice(&instrs[0].to_le_bytes());
            patch[4..8].copy_from_slice(&NOP.to_le_bytes());
            patch[8..12].copy_from_slice(&NOP.to_le_bytes());
        } else {
            patch[0..4].copy_from_slice(&instrs[0].to_le_bytes());
            patch[4..8].copy_from_slice(&instrs[1].to_le_bytes());
            patch[8..12].copy_from_slice(&instrs[2].to_le_bytes());
        }
    }

    // Non-macOS: use B imm26 if in range (±128 MiB), else panic.
    #[cfg(not(target_os = "macos"))]
    {
        // imm26 is a signed word offset (×4): range = [-2^25, 2^25-1] words → ±128 MiB.
        const MIN_WORDS: isize = -(1isize << 25);
        const MAX_WORDS: isize = (1isize << 25) - 1;

        let offset_words = (jit_addr as isize - func_addr as isize) / 4;
        if offset_words < MIN_WORDS || offset_words > MAX_WORDS {
            panic!(
                "JIT memory is out of branch range for `B imm26`: offset_words = {offset_words}, expected ±2^25 words (±128 MiB)"
            );
        }

        let branch_instr: u32 = 0x14000000 | ((offset_words as u32) & 0x03FF_FFFF);
        patch[0..4].copy_from_slice(&branch_instr.to_le_bytes());
        patch[4..8].copy_from_slice(&NOP.to_le_bytes());
        patch[8..12].copy_from_slice(&NOP.to_le_bytes());
    }

    unsafe {
        patch_function(src.as_ptr() as *mut u8, &patch);
    }

    PatchGuard::new(
        src.as_ptr() as *mut u8,
        original_bytes.to_vec(),
        PATCH_SIZE,
        jit_memory,
        jit_size,
    )
}



// ================= Linux-only preflight helpers (AArch64) =================

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
unsafe fn assert_min_patch_window_or_panic(
    src: &crate::injector_core::common::FuncPtrInternal,
    patch_size: usize,
) {
    assert!(
        patch_size % 4 == 0,
        "PATCH_SIZE must be a multiple of 4 on AArch64 (got {patch_size})"
    );

    // Read exactly what we intend to overwrite.
    let buf = read_bytes(src.as_ptr() as *mut u8, patch_size);

    for (idx, chunk) in buf.chunks_exact(4).enumerate() {
        let w = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);

        // Allow a top-of-entry veneer (unconditional B) at +0 only.
        if idx == 0 && is_a64_b_uncond_imm(w) {
            continue;
        }

        // Refuse to patch if we see any hard, non-fallthrough terminators:
        //  - RET anywhere in the patch window
        //  - BR Xn anywhere in the patch window (indirect tail)
        //  - B imm26 if it appears after the first instruction
        if is_a64_ret(w) || is_a64_br_reg(w) || is_a64_b_uncond_imm(w) {
            let at = idx * 4;
            panic!(
                "Target function too small: terminator in first {patch_size} bytes (at +{:#x}). \
                 Refusing to patch to avoid UB.",
                at
            );
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[inline]
fn is_a64_ret(w: u32) -> bool {
    // RET Xn: 1101 0110 0101 1111 0000 00 Rn 00000
    const MASK: u32 = 0xFFFF_FC1F;
    const BASE: u32 = 0xD65F_0000;
    (w & MASK) == BASE
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[inline]
fn is_a64_br_reg(w: u32) -> bool {
    // BR Xn: 1101 0110 0001 1111 0000 00 Rn 00000
    const MASK: u32 = 0xFFFF_FC1F;
    const BASE: u32 = 0xD61F_0000;
    (w & MASK) == BASE
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[inline]
fn is_a64_b_uncond_imm(w: u32) -> bool {
    // B imm26: top 6 bits == 000101
    (w & 0x7C00_0000) == 0x1400_0000
}

