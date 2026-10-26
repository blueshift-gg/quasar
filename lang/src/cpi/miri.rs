//! Read the real CPI buffers at the syscall boundary, without an SVM.

use core::{cell::Cell, slice};

use super::*;
use crate::{
    account::Account,
    miri::{Counter, with_accounts},
    wrapper::{Program, SystemProgram, UncheckedAccount},
};

type Invoke = unsafe fn(*const u8, *const u8, u64, *const u8, u64);

std::thread_local! {
    pub(crate) static INVOKE: Cell<Option<Invoke>> = const { Cell::new(None) };
}

fn with_invoke(invoke: Invoke, run: impl FnOnce()) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            INVOKE.set(None);
        }
    }
    assert!(INVOKE.replace(Some(invoke)).is_none());
    let _reset = Reset;
    run();
}

/// # Safety
/// All arguments follow the CPI ABI and stay alive until this returns.
unsafe fn inspect(
    instruction: *const u8,
    infos: *const u8,
    infos_len: u64,
    signers: *const u8,
    signers_len: u64,
) {
    // SAFETY: the call built these initialized structs, arrays and slices.
    unsafe {
        let instruction = &*instruction.cast::<SolInstruction>();
        assert_eq!(*instruction.program_id, <SystemProgram as crate::wrapper::ProgramId>::ID);
        assert_eq!(instruction.accounts_len, infos_len);
        // The two-account case signs with one PDA; the empty call has no signers.
        assert_eq!(signers_len, u64::from(infos_len != 0));
        assert_eq!(slice::from_raw_parts(instruction.data, instruction.data_len as usize), &[42]);
        let metas = slice::from_raw_parts(instruction.accounts, infos_len as usize);
        let infos = slice::from_raw_parts(infos.cast::<SolAccountInfo>(), infos_len as usize);
        for (index, (meta, info)) in metas.iter().zip(infos).enumerate() {
            assert_eq!(*meta.pubkey, *info.key);
            assert_eq!(*info.owner, crate::miri::OWNER);
            assert_eq!(info.flags, 0x010101);
            assert_eq!(info.rent_epoch, 0);
            assert_eq!(*info.lamports, 100);
            assert!(meta.is_signer);
            assert_eq!(meta.is_writable, index == 0);
            let data = slice::from_raw_parts(info.data, info.data_len as usize);
            assert_eq!(data, &[9, 0, 0, 0, 0, 0, 0, 0, 0]);
            if meta.is_writable {
                *info.lamports += 1;
                // All shared data reads above are finished before the simulated callee writes.
                info.data.add(1).write(23);
            }
        }
        for signer in slice::from_raw_parts(signers.cast::<RawSlice>(), signers_len as usize) {
            let seeds = slice::from_raw_parts(signer.addr.cast::<RawSlice>(), signer.len as usize);
            assert_eq!(seeds.len(), 3);
            for (seed, expected) in seeds.iter().zip([&b"seed"[..], &b""[..], &[254]]) {
                assert_eq!(slice::from_raw_parts(seed.addr, seed.len as usize), expected);
            }
        }
    }
}

#[test]
fn cpi_buffers_keep_writable_and_readonly_accounts_separate() {
    let data = [9, 0, 0, 0, 0, 0, 0, 0, 0];
    for variable in [false, true] {
        with_accounts(&[(1, &[]), (2, &data), (3, &data)], &[], |cursor| {
            let program = cursor.load::<Program<SystemProgram>>(false).unwrap();
            let mut writable = cursor.load::<Account<Counter>>(true).unwrap();
            let readonly = cursor.load::<UncheckedAccount>(false).unwrap();
            with_invoke(inspect, || {
                let call = Cpi::new(
                    &program,
                    [
                        CpiAccount::writable(&mut writable).signer(),
                        CpiAccount::readonly(&readonly).signer(),
                    ],
                    [42],
                );
                let signer = SignerSeeds { seeds: [b"seed", b""], bump: [254] };
                if variable {
                    call.with_data_len(1).unwrap().invoke_signed(&signer).unwrap();
                } else {
                    call.invoke_signed(&signer).unwrap();
                }
            });
            // Writable typed accounts must restore and recheck their poisoned handle.
            writable.reload().unwrap();
            assert_eq!(writable.value(), 23);
            assert_eq!(writable.lamports(), 101);
            assert_eq!(readonly.data(), &data);
            assert_eq!(readonly.lamports(), 100);
        });
    }
}

#[test]
fn cpi_buffers_allow_zero_accounts_and_no_signers() {
    with_accounts(&[(1, &[])], &[], |cursor| {
        let program = cursor.load::<Program<SystemProgram>>(false).unwrap();
        with_invoke(inspect, || Cpi::new(&program, [], [42]).invoke().unwrap());
    });
}

#[test]
fn variable_data_sends_only_its_prefix_and_rejects_oversize() {
    with_accounts(&[(1, &[])], &[], |cursor| {
        let program = cursor.load::<Program<SystemProgram>>(false).unwrap();
        assert!(Cpi::new(&program, [], [42]).with_data_len(2).is_err());
        with_invoke(inspect, || {
            Cpi::new(&program, [], [42, 99, 99]).with_data_len(1).unwrap().invoke().unwrap();
        });
    });
}

#[test]
fn payer_may_also_be_a_readonly_authority_while_its_key_is_borrowed() {
    unsafe fn pay(_: *const u8, infos: *const u8, count: u64, _: *const u8, _: u64) {
        assert_eq!(count, 2);
        // SAFETY: the call passed two initialized infos pointing to the same payer.
        unsafe {
            let infos = slice::from_raw_parts(infos.cast::<SolAccountInfo>(), 2);
            assert_eq!(infos[0].key, infos[1].key);
            *infos[0].lamports -= 1;
            assert_eq!(*infos[1].lamports, 99);
        }
    }
    with_accounts(&[(1, &[]), (2, &[])], &[], |cursor| {
        let program = cursor.load::<Program<SystemProgram>>(false).unwrap();
        let payer = cursor.load::<UncheckedAccount>(true).unwrap();
        let key = payer.address();
        with_invoke(pay, || {
            // SAFETY: the simulated callee only debits lamports.
            let paying = unsafe { CpiAccount::paying(&payer) };
            Cpi::new(&program, [paying, CpiAccount::readonly(&payer).signer()], [])
                .invoke()
                .unwrap();
        });
        assert_eq!(*key, Address::new_from_array([2; 32]));
        assert_eq!(payer.lamports(), 99);
    });
}
