//! Errors the framework itself raises, beyond the runtime's own `ProgramError`
//! variants. The codes are part of every program's public interface.

use solana_program_error::ProgramError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Error {
    /// An account appears twice where the accounts struct needs distinct accounts.
    DuplicateAccount = 3000,
    /// An account that must be a program is not executable.
    NotExecutable = 3001,
    /// An account's data is shorter than its kind's layout, or longer than a fixed one.
    DataLength = 3002,
    /// An account's data does not start with its kind's discriminator.
    Discriminator = 3003,
    /// An account's stored key differs from the account named by `has_one`.
    HasOne = 3004,
    /// A `check(..)` without its own error does not hold.
    Constraint = 3005,
    /// An account is not at the address `address = ..` names.
    Address = 3006,
    /// An account is not the sysvar `Sysvar<S>` names.
    Sysvar = 3007,
}

impl From<Error> for ProgramError {
    fn from(error: Error) -> Self {
        ProgramError::Custom(error as u32)
    }
}

// What the `debug` build says when a check fails, laid out as rustc lays out
// an error, for whoever reads the log, a person or a model:
//
//     error[IllegalOwner]: account `purse` is owned by 1111.., expected Ac2H..
//        --> src/lib.rs:293:16
//         |
//     293 |     pub purse: Account<Purse>,
//         |                ^^^^^^^^^^^^^^ account #2 of the instruction
//         |
//         = help: ...
//
// The check that fails writes the first line, since it has the values; the
// accounts struct supplies where it was declared (`At`), and the check its
// help. Without the feature all of it is nothing, and none of its text is in
// the program. The feature is this crate's, whichever crate calls.

/// Where an account or one of its constraints is declared: the account's
/// name, and the lines that show the source, from `-->` to the last `|`.
pub struct At {
    pub name: &'static str,
    pub lines: &'static [&'static str],
}

impl At {
    /// An account no accounts struct names, as one of a `Remaining`.
    pub const UNNAMED: Self = Self { name: "", lines: &[] };
}

/// `error[code]: account `name` `, to continue with what is wrong with it.
pub fn head(code: &str, name: &str) -> Line {
    let line = Line::new().text("error[").text(code).text("]: account ");
    if name.is_empty() { line } else { line.text("`").text(name).text("` ") }
}

/// Logs where the error was declared, then how to fix it.
#[cold]
#[inline(never)]
pub fn located(at: &At, help: &str) {
    for line in at.lines {
        crate::syscalls::log(line);
    }
    if !help.is_empty() {
        Line::new().text("     = help: ").text(help).log();
    }
}

/// `result`, after logging `error` and `located(at, help)` if it is an error.
#[inline(always)]
pub fn explain<T>(
    result: crate::Result<T>,
    error: &'static str,
    at: &At,
    help: &'static str,
) -> crate::Result<T> {
    if cfg!(feature = "debug") && result.is_err() {
        crate::syscalls::log(error);
        located(at, help);
    }
    result
}

/// `explain`, for a check that an address stored or passed is another: the
/// error is `before`, `found`, `between`, `expected`.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub fn explain_same<T>(
    result: crate::Result<T>,
    before: &'static str,
    found: &crate::Address,
    between: &'static str,
    expected: &crate::Address,
    at: &At,
    help: &'static str,
) -> crate::Result<T> {
    if cfg!(feature = "debug") && result.is_err() {
        differ(before, found, between, expected);
        located(at, help);
    }
    result
}

/// `explain`, for a library's constraint: `why` logs the error with what it
/// found, and gives the help.
#[inline(always)]
pub fn explain_with<T>(
    result: crate::Result<T>,
    why: impl FnOnce() -> &'static str,
    at: &At,
) -> crate::Result<T> {
    if cfg!(feature = "debug") && result.is_err() {
        let help = why();
        located(at, help);
    }
    result
}

/// `why` for a library constraint without one of its own: `Check::why(&self,
/// account, name)` logs what the check found and gives the help.
pub trait Unexplained {
    fn why<A>(&self, _account: &A, name: &str) -> &'static str {
        head("Constraint", name).text("does not satisfy this constraint").log();
        ""
    }
}

impl<T> Unexplained for T {}

// Out of line, in a frame of its own: a program's handlers share one, which
// a line's buffer per check would overflow.
#[cold]
#[inline(never)]
fn differ(before: &str, found: &crate::Address, between: &str, expected: &crate::Address) {
    Line::new().text(before).key(found).text(between).key(expected).log();
}

/// One line of the log, written in place: text, addresses in base 58 and
/// numbers in decimal. What does not fit is left out. Built only where the
/// `debug` feature is on, and out of line (`#[cold]`, `#[inline(never)]`).
pub struct Line {
    bytes: [u8; 280],
    len: usize,
}

impl Default for Line {
    fn default() -> Self {
        Self::new()
    }
}

impl Line {
    pub fn new() -> Self {
        Self { bytes: [0; 280], len: 0 }
    }

    pub fn text(mut self, text: &str) -> Self {
        for byte in text.bytes() {
            self.push(byte);
        }
        self
    }

    fn push(&mut self, byte: u8) {
        if let Some(slot) = self.bytes.get_mut(self.len) {
            *slot = byte;
            self.len += 1;
        }
    }

    pub fn number(mut self, mut number: u64) -> Self {
        let mut digits = [0u8; 20];
        let mut count = 0;
        loop {
            digits[count] = b'0' + (number % 10) as u8;
            count += 1;
            number /= 10;
            if number == 0 {
                break;
            }
        }
        for digit in digits[..count].iter().rev() {
            self.push(*digit);
        }
        self
    }

    /// `bytes`, each in decimal: `[1, 0, 255]`.
    pub fn bytes(mut self, bytes: &[u8]) -> Self {
        self = self.text("[");
        for (index, byte) in bytes.iter().enumerate() {
            self = self.text(if index == 0 { "" } else { ", " }).number((*byte).into());
        }
        self.text("]")
    }

    /// Each of `keys`, with "or" between them.
    pub fn keys(mut self, keys: &[crate::Address]) -> Self {
        for (index, key) in keys.iter().enumerate() {
            self = self.text(if index == 0 { "" } else { " or " }).key(key);
        }
        self
    }

    /// `key` in base 58, as explorers and clients show it.
    pub fn key(mut self, key: &crate::Address) -> Self {
        const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
        // Base 58 digits, least significant first; 32 bytes take at most 44.
        let mut digits = [0u8; 44];
        let mut count = 0;
        for byte in key.as_array() {
            let mut carry = u32::from(*byte);
            for digit in &mut digits[..count] {
                carry += u32::from(*digit) << 8;
                *digit = (carry % 58) as u8;
                carry /= 58;
            }
            while carry > 0 {
                digits[count] = (carry % 58) as u8;
                count += 1;
                carry /= 58;
            }
        }
        for _ in key.as_array().iter().take_while(|byte| **byte == 0) {
            self.push(b'1');
        }
        for digit in digits[..count].iter().rev() {
            self.push(ALPHABET[usize::from(*digit)]);
        }
        self
    }

    pub fn log(self) {
        // Whole characters, unless the line was cut short inside one.
        let line = &self.bytes[..self.len];
        let line = match core::str::from_utf8(line) {
            Ok(line) => line,
            Err(error) => core::str::from_utf8(&line[..error.valid_up_to()]).unwrap_or(""),
        };
        crate::syscalls::log(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_writes_addresses_as_base_58() {
        extern crate std;
        let mixed = core::array::from_fn(|index| (index * 37) as u8);
        for bytes in [[0; 32], [1; 32], [255; 32], mixed] {
            let key = crate::Address::new_from_array(bytes);
            let line = Line::new().text("k ").key(&key).text(" ").number(1050);
            let written = core::str::from_utf8(&line.bytes[..line.len]).unwrap();
            assert_eq!(written, std::format!("k {key} 1050"));
        }
    }
}
