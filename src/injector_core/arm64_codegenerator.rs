#![cfg(target_arch = "aarch64")]

use crate::injector_core::utils::*;

/// Insert `value` into a bit field of `u32` with range [lsb..=msb].
#[inline]
fn set_bits(value: u32, lsb: u8, msb: u8) -> u32 {
    debug_assert!(msb < 32 && lsb <= msb);
    debug_assert!(value < (1 << (msb - lsb + 1)));
    value << lsb
}

/// Extracts a u16 from `start` bit of a 64-bit value.
#[inline]
fn extract16(src: u64, start: usize) -> u16 {
    ((src >> start) & 0xFFFF) as u16
}

/// Emit RET instruction with x30 as destination register.
pub fn emit_ret_x30() -> [bool; 32] {
    emit_ret(&u8_to_bits_5(30))
}

/// Emit RET instruction with a specified register.
pub fn emit_ret(register_name: &[bool; 5]) -> [bool; 32] {
    let reg_bits = bits_to_u8_5(register_name);
    let insn = 0b11010110010111100000000000000000u32 | set_bits(reg_bits as u32, 5, 9);
    u32_to_bits(insn)
}

/// Emit BR instruction to jump to a register.
pub fn emit_br(register_name: [bool; 5]) -> [bool; 32] {
    let reg_bits = bits_to_u8_5(&register_name);
    let insn = 0b11010110000111110000000000000000u32 | set_bits(reg_bits as u32, 5, 9);
    u32_to_bits(insn)
}

/// Emit MOVK with immediate extracted from address.
pub fn emit_movk_from_address(
    address: u64,
    start: usize,
    sf: bool,
    hw: [bool; 2],
    register_name: [bool; 5],
) -> [bool; 32] {
    let imm16 = extract16(address, start);
    emit_movk(u16_to_bits(imm16), sf, hw, register_name)
}

/// Emit MOVK instruction.
pub fn emit_movk(
    value_bits: [bool; 16],
    sf: bool,
    hw: [bool; 2],
    register_name: [bool; 5],
) -> [bool; 32] {
    let rd = bits_to_u8_5(&register_name) as u32;
    let imm = bits_to_u16(&value_bits) as u32;
    let hw_val = bits_to_u2(&hw) as u32;

    let insn = (sf as u32) << 31
        | set_bits(0b111101, 25, 30)
        | set_bits(hw_val, 21, 22)
        | set_bits(imm, 5, 20)
        | set_bits(0b101001, 10, 15)
        | set_bits(0b11, 23, 24)
        | set_bits(rd, 0, 4);

    u32_to_bits(insn)
}

/// Emit MOVZ with immediate extracted from address.
pub fn emit_movz_from_address(
    address: u64,
    start: usize,
    sf: bool,
    hw: [bool; 2],
    register_name: [bool; 5],
) -> [bool; 32] {
    let imm16 = extract16(address, start);
    emit_movz(u16_to_bits(imm16), sf, hw, register_name)
}

/// Emit MOVZ instruction.
pub fn emit_movz(
    value_bits: [bool; 16],
    sf: bool,
    hw: [bool; 2],
    register_name: [bool; 5],
) -> [bool; 32] {
    let rd = bits_to_u8_5(&register_name) as u32;
    let imm = bits_to_u16(&value_bits) as u32;
    let hw_val = bits_to_u2(&hw) as u32;

    let insn = (sf as u32) << 31
        | set_bits(0b110101, 25, 30)
        | set_bits(hw_val, 21, 22)
        | set_bits(imm, 5, 20)
        | set_bits(0b101001, 10, 15)
        | set_bits(0b01, 23, 24)
        | set_bits(rd, 0, 4);

    u32_to_bits(insn)
}

/// Convert u32 to [bool; 32] (big-endian).
pub fn u32_to_bits(v: u32) -> [bool; 32] {
    let mut out = [false; 32];
    for i in 0..32 {
        out[31 - i] = (v >> i) & 1 != 0;
    }
    out
}

/// Convert u16 to [bool; 16] (big-endian).
pub fn u16_to_bits(v: u16) -> [bool; 16] {
    let mut out = [false; 16];
    for i in 0..16 {
        out[15 - i] = (v >> i) & 1 != 0;
    }
    out
}

/// Convert [bool; 16] to u16 (little-endian).
pub fn bits_to_u16(bits: &[bool; 16]) -> u16 {
    let mut val = 0;
    for (i, b) in bits.iter().rev().enumerate() {
        if *b {
            val |= 1 << i;
        }
    }
    val
}

/// Convert [bool; 5] to u8.
fn bits_to_u8_5(bits: &[bool; 5]) -> u8 {
    let mut val = 0;
    for (i, b) in bits.iter().rev().enumerate() {
        if *b {
            val |= 1 << i;
        }
    }
    val
}

/// Convert [bool; 2] to u8.
fn bits_to_u2(bits: &[bool; 2]) -> u8 {
    ((bits[0] as u8) << 1) | (bits[1] as u8)
}

/// Convert u8 to [bool; 5].
fn u8_to_bits_5(v: u8) -> [bool; 5] {
    let mut out = [false; 5];
    for i in 0..5 {
        out[4 - i] = (v >> i) & 1 != 0;
    }
    out
}
