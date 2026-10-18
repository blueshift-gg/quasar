//! The SPL Token program: its account layouts, its calls, and the `token(..)`
//! and `mint(..)` constraints.

use core::ops::Deref;

use quasar_lang::zeropod::{self, Layout, Plain, ZeroPod};

use quasar_lang::{
    Address, ProgramError, Result,
    account::{InterfaceKind, Kind},
    cpi::{self, Argument, Cpi, CpiAccount, Enforced, Enforces, Named},
    error::head,
    input::{self, Entry},
    pda::SignerSeeds,
    system,
    sysvar::RentSource,
    wrapper::{Callee, Program, ProgramId, ProgramIds, Signer, SystemProgram, Wrapper},
};

/// The SPL Token program.
pub struct Token;

impl ProgramId for Token {
    const ID: Address = Address::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
}

/// The Token-2022 program.
pub struct Token2022;

impl ProgramId for Token2022 {
    const ID: Address = Address::from_str_const("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
}

/// Token or Token-2022, for `Interface<TokenInterface>` and
/// `InterfaceAccount<TokenAccount>`.
pub struct TokenInterface;

impl ProgramIds for TokenInterface {
    const IDS: &'static [Address] = &[Token::ID, Token2022::ID];
}

/// Token-2022's rule for data past the base layout: the whole account at
/// least one byte past `TokenAccount`'s length, zeros up to it, then the
/// account type (1 mint, 2 token account), then extensions.
fn extension_layout_holds(data: &[u8], base: usize, account_type: u8) -> bool {
    const TYPE_AT: usize = size::<TokenAccount>();
    const MULTISIG_LEN: usize = 355;
    data.len() == base
        || (data.len() > TYPE_AT
            && data.len() != MULTISIG_LEN
            && data[base..TYPE_AT].iter().all(|byte| *byte == 0)
            && data[TYPE_AT] == account_type)
}

/// A base layout's size, which each has.
const fn size<T: ZeroPod>() -> usize {
    match T::SIZE {
        Some(size) => size,
        None => panic!("an SPL layout has one size"),
    }
}

/// A value the Token program may leave unset: a little-endian `u32` tag, 0
/// or 1, then room for the value either way. Not Borsh's `Option`, so stored
/// by hand.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct COption<T>(pub Option<T>);

// SAFETY: `check` accepts a tag of 0 or 1 and, after a 1, a valid `T`, in
// `4 + T`'s size; `read` and `write` follow the tag, and `write` zeroes the
// room of an unset value.
unsafe impl<T: Plain> ZeroPod for COption<T> {
    type Ref<'a>
        = Option<T::Ref<'a>>
    where
        Self: 'a;
    type In<'a>
        = Option<T::In<'a>>
    where
        Self: 'a;
    const SIZE: Option<usize> = Some(4 + size::<T>());

    fn check(bytes: &[u8], _: &[usize]) -> core::result::Result<usize, zeropod::Error> {
        let (tag, value) = bytes.split_first_chunk::<4>().ok_or(zeropod::Error::TooShort)?;
        match u32::from_le_bytes(*tag) {
            0 if value.len() >= size::<T>() => Ok(4 + size::<T>()),
            1 => T::check(value, &[]).map(|size| 4 + size),
            0 => Err(zeropod::Error::TooShort),
            _ => Err(zeropod::Error::InvalidTag),
        }
    }

    unsafe fn len(_: &[u8]) -> usize {
        4 + size::<T>()
    }

    unsafe fn read(bytes: &[u8]) -> Self::Ref<'_> {
        // SAFETY: a valid encoding starts with its tag, then the value if 1.
        unsafe { (*bytes.get_unchecked(0) == 1).then(|| T::read(bytes.get_unchecked(4..))) }
    }

    fn encoded_len(_: &Self::In<'_>, _: &[usize]) -> core::result::Result<usize, zeropod::Error> {
        Ok(4 + size::<T>())
    }

    unsafe fn write(value: &Self::In<'_>, out: &mut [u8]) -> usize {
        // SAFETY: `out` holds the tag and the room for the value.
        unsafe {
            let (tag, room) = out.split_at_mut_unchecked(4);
            tag.copy_from_slice(&u32::from(value.is_some()).to_le_bytes());
            match value {
                Some(value) => _ = T::write(value, room),
                None => room.get_unchecked_mut(..size::<T>()).fill(0),
            }
        }
        4 + size::<T>()
    }

    fn max_len(_: &[usize]) -> Option<usize> {
        Some(4 + size::<T>())
    }

    fn input(&self) -> Self::In<'_> {
        self.0.as_ref().map(|value| value.input())
    }

    fn own(value: Self::Ref<'_>) -> Self {
        Self(value.map(|value| T::own(value)))
    }
}

/// `spl_token::state::Account`.
#[derive(ZeroPod, Clone, Copy, Debug, PartialEq, Eq)]
#[zeropod(crate = quasar_lang::zeropod)]
pub struct TokenAccount {
    pub mint: Address,
    pub owner: Address,
    pub amount: u64,
    pub delegate: COption<Address>,
    /// 1 initialized, 2 frozen.
    pub state: u8,
    pub is_native: COption<u64>,
    pub delegated_amount: u64,
    pub close_authority: COption<Address>,
}

impl Kind for TokenAccount {
    const OWNER: Address = Token::ID;
    const DISCRIMINATOR: &'static [u8] = &[];
    const PROGRAM_OWNED: bool = false;

    #[cfg(not(target_os = "solana"))]
    const IDL: Option<quasar_lang::client::IdlKind> = idl_kind(
        "TokenAccount",
        size::<TokenAccount>(),
        &[
            field("mint", "Address"),
            field("owner", "Address"),
            field("amount", "u64"),
            field("delegate", "COption<Address>"),
            field("state", "u8"),
            field("is_native", "COption<u64>"),
            field("delegated_amount", "u64"),
            field("close_authority", "COption<Address>"),
        ],
    );

    /// Beyond the layout, what the Token program's unpacking accepts.
    fn validate(account: &TokenAccountView) -> bool {
        matches!(account.state(), 1 | 2)
    }
}

impl InterfaceKind for TokenAccount {
    type Owners = TokenInterface;
    // SAFETY: its fields are addresses, integers and tags compared with 1,
    // which read from any bytes.
    const CHECK: Option<Named> = Some(unsafe { Named::new(TOKEN_ACCOUNT_CHECK) });

    /// Exactly what Token-2022's `StateWithExtensions::unpack` accepts.
    fn validate(data: &[u8]) -> bool {
        let base = size::<Self>();
        data.len() >= base
            && Self::view(&data[..base]).is_ok_and(<Self as Kind>::validate)
            && extension_layout_holds(data, base, 2)
    }

    fn why(data: &[u8], name: &str) -> &'static str {
        head("InvalidAccountData", name).text("is not an initialized token account").log();
        match data.get(108) {
            Some(0) => {
                "the Token program owns it but `InitializeAccount` never ran on it: create the token account first, or declare the field `init(..), token(..)`"
            }
            _ => {
                "the Token program owns it, but it holds something else, a mint say: check the address passed"
            }
        }
    }
}

/// `spl_token::state::Mint`.
#[derive(ZeroPod, Clone, Copy, Debug, PartialEq, Eq)]
#[zeropod(crate = quasar_lang::zeropod)]
pub struct Mint {
    pub mint_authority: COption<Address>,
    pub supply: u64,
    pub decimals: u8,
    pub is_initialized: bool,
    pub freeze_authority: COption<Address>,
}

impl Kind for Mint {
    const OWNER: Address = Token::ID;
    const DISCRIMINATOR: &'static [u8] = &[];
    const PROGRAM_OWNED: bool = false;

    #[cfg(not(target_os = "solana"))]
    const IDL: Option<quasar_lang::client::IdlKind> = idl_kind(
        "Mint",
        size::<Mint>(),
        &[
            field("mint_authority", "COption<Address>"),
            field("supply", "u64"),
            field("decimals", "u8"),
            field("is_initialized", "bool"),
            field("freeze_authority", "COption<Address>"),
        ],
    );

    /// Beyond the layout, what the Token program's unpacking accepts.
    fn validate(mint: &MintView) -> bool {
        mint.is_initialized()
    }
}

impl InterfaceKind for Mint {
    type Owners = TokenInterface;
    // SAFETY: as for a token account; its flag is read as "not zero".
    const CHECK: Option<Named> = Some(unsafe { Named::new(MINT_CHECK) });

    /// Exactly what Token-2022's `StateWithExtensions::unpack` accepts.
    fn validate(data: &[u8]) -> bool {
        let base = size::<Self>();
        data.len() >= base
            && Self::view(&data[..base]).is_ok_and(<Self as Kind>::validate)
            && extension_layout_holds(data, base, 1)
    }

    fn why(_: &[u8], name: &str) -> &'static str {
        head("InvalidAccountData", name).text("is not an initialized mint").log();
        "the Token program owns it, but it holds something else, a token account say, or `InitializeMint` never ran on it: check the address passed"
    }
}

#[cfg(not(target_os = "solana"))]
const fn idl_kind(
    name: &'static str,
    size: usize,
    fields: &'static [quasar_lang::client::IdlField],
) -> Option<quasar_lang::client::IdlKind> {
    Some(quasar_lang::client::IdlKind { name, discriminator: &[], size: Some(size), fields })
}

#[cfg(not(target_os = "solana"))]
const fn field(name: &'static str, ty: &'static str) -> quasar_lang::client::IdlField {
    quasar_lang::client::IdlField { name, ty }
}

/// A Token program owns the account, which holds an initialized token account.
pub const TOKEN_ACCOUNT_CHECK: cpi::Check = "spl_token::account";
/// A Token program owns the account, which holds an initialized mint.
pub const MINT_CHECK: cpi::Check = "spl_token::mint";
/// The token account's mint is the one named: `token(mint = ..)`.
pub const TOKEN_MINT_CHECK: cpi::Check = "spl_token::account.mint";
/// The one named may move the account's tokens: `token(authority = ..)`
/// requires its owner, and a transfer its owner or the delegate it approved,
/// so a field a transfer debits accepts either.
pub const TOKEN_AUTHORITY_CHECK: cpi::Check = "spl_token::account.owner";

impl Check<'_> {
    /// The check each key makes.
    pub const KEYS: &'static [(&'static str, cpi::Check)] =
        &[("mint", TOKEN_MINT_CHECK), ("authority", TOKEN_AUTHORITY_CHECK)];
}

/// The call is one to a Token program, whose checks `ENFORCES` names.
const fn is_token_program<C: Callee>() {
    assert!(cpi::within(C::Ids::IDS, TokenInterface::IDS), "the Token programs' instruction");
}

/// What each call does with each argument (`cpi::Enforces`): the checks
/// Token, p-token and Token-2022 all make before the instruction succeeds,
/// and what it leaves of the account.
pub mod transfer {
    use super::*;
    // SAFETY: `Transfer` assigns and resizes nothing, and writes only amounts.
    pub const ENFORCES: Enforces = unsafe {
        &[
            Argument::NONE,
            // The authority signs as the account's owner or its delegate.
            Argument::kept(&[
                Enforced::just(TOKEN_ACCOUNT_CHECK),
                Enforced::of(TOKEN_AUTHORITY_CHECK, 3),
            ]),
            Argument::kept(&[Enforced::just(TOKEN_ACCOUNT_CHECK)]),
        ]
    };
}

pub mod mint_to {
    use super::*;
    // SAFETY: `MintTo` assigns and resizes nothing, and writes only amounts.
    pub const ENFORCES: Enforces = unsafe {
        &[
            Argument::NONE,
            Argument::kept(&[Enforced::just(MINT_CHECK)]),
            Argument::kept(&[
                Enforced::just(TOKEN_ACCOUNT_CHECK),
                Enforced::of(TOKEN_MINT_CHECK, 1),
            ]),
        ]
    };
}

pub mod burn {
    use super::*;
    // SAFETY: `Burn` assigns and resizes nothing, and writes only amounts.
    pub const ENFORCES: Enforces = unsafe {
        &[
            Argument::NONE,
            Argument::kept(&[
                Enforced::just(TOKEN_ACCOUNT_CHECK),
                Enforced::of(TOKEN_MINT_CHECK, 2),
            ]),
            Argument::kept(&[Enforced::just(MINT_CHECK)]),
        ]
    };
}

/// The account is closed; a mint with a close authority closes too, so the
/// call checks nothing a token account's load does.
pub mod close_account {
    use super::*;
    // SAFETY: the destination only receives lamports.
    pub const ENFORCES: Enforces =
        &[Argument::NONE, Argument::closed(&[]), unsafe { Argument::kept(&[]) }];
}

/// `TokenInstruction` variants: the index byte, then the fields.
const TRANSFER: u8 = 3;
const MINT_TO: u8 = 7;
const BURN: u8 = 8;
const CLOSE_ACCOUNT: u8 = 9;
const INITIALIZE_ACCOUNT_3: u8 = 18;
const INITIALIZE_MINT_2: u8 = 20;

/// Moves `amount` tokens; `authority` signs. The Token program keeps both
/// accounts token accounts (`CpiAccount::kept`).
pub fn transfer<'a, C: Callee, F: Wrapper, T: Wrapper, A: Wrapper>(
    token_program: &'a C,
    from: &'a mut F,
    to: &'a mut T,
    authority: &'a A,
    amount: u64,
) -> Cpi<'a, C::Ids, 3, 9> {
    let from = CpiAccount::argument(transfer::ENFORCES, 1, from);
    let to = CpiAccount::argument(transfer::ENFORCES, 2, to);
    let accounts = [from, to, CpiAccount::readonly(authority).signer()];
    const { is_token_program::<C>() };
    Cpi::new(token_program, accounts, with_amount(TRANSFER, amount))
}

/// Creates `amount` tokens of `mint` in `to`; the mint's authority signs.
pub fn mint_to<'a, C: Callee, M: Wrapper, T: Wrapper, A: Wrapper>(
    token_program: &'a C,
    mint: &'a mut M,
    to: &'a mut T,
    authority: &'a A,
    amount: u64,
) -> Cpi<'a, C::Ids, 3, 9> {
    let mint = CpiAccount::argument(mint_to::ENFORCES, 1, mint);
    let to = CpiAccount::argument(mint_to::ENFORCES, 2, to);
    let accounts = [mint, to, CpiAccount::readonly(authority).signer()];
    const { is_token_program::<C>() };
    Cpi::new(token_program, accounts, with_amount(MINT_TO, amount))
}

/// Destroys `amount` of the tokens `from` holds; its owner signs.
pub fn burn<'a, C: Callee, F: Wrapper, M: Wrapper, A: Wrapper>(
    token_program: &'a C,
    from: &'a mut F,
    mint: &'a mut M,
    authority: &'a A,
    amount: u64,
) -> Cpi<'a, C::Ids, 3, 9> {
    let from = CpiAccount::argument(burn::ENFORCES, 1, from);
    let mint = CpiAccount::argument(burn::ENFORCES, 2, mint);
    let accounts = [from, mint, CpiAccount::readonly(authority).signer()];
    const { is_token_program::<C>() };
    Cpi::new(token_program, accounts, with_amount(BURN, amount))
}

/// An instruction whose one field is an amount.
fn with_amount(instruction: u8, amount: u64) -> [u8; 9] {
    let mut data = [instruction; 9];
    data[1..].copy_from_slice(&amount.to_le_bytes());
    data
}

/// Closes a token account, which must hold no tokens, sending its lamports to
/// `destination`; `authority` signs. The account is taken by value: it is
/// gone once the call returns.
pub fn close_account<'a, C: Callee, W: Wrapper, D: Wrapper, A: Wrapper>(
    token_program: &'a C,
    account: W,
    destination: &'a mut D,
    authority: &'a A,
) -> Cpi<'a, C::Ids, 3, 1> {
    let accounts = [
        CpiAccount::closing(account),
        CpiAccount::argument(close_account::ENFORCES, 2, destination),
        CpiAccount::readonly(authority).signer(),
    ];
    Cpi::new(token_program, accounts, [CLOSE_ACCOUNT])
}

/// Initializes a created token account for `mint`, owned by `owner`.
fn initialize_account3<'a, C: Callee, W: Wrapper>(
    token_program: &'a C,
    account: &'a mut W,
    mint: &'a Entry,
    owner: &Address,
) -> Cpi<'a, C::Ids, 2, 33> {
    let mut data = [INITIALIZE_ACCOUNT_3; 33];
    data[1..].copy_from_slice(owner.as_ref());
    let accounts = [CpiAccount::writable(account), CpiAccount::readonly_entry(mint)];
    Cpi::new(token_program, accounts, data)
}

/// Initializes a created mint with `decimals`, minted by `authority`, with
/// no freeze authority.
fn initialize_mint2<'a, C: Callee, W: Wrapper>(
    token_program: &'a C,
    mint: &'a mut W,
    decimals: u8,
    authority: &Address,
) -> Cpi<'a, C::Ids, 1, 35> {
    let mut data = [0; 35];
    data[0] = INITIALIZE_MINT_2;
    data[1] = decimals;
    data[2..34].copy_from_slice(authority.as_ref());
    Cpi::new(token_program, [CpiAccount::writable(mint)], data)
}

/// Errors of the `token(..)` and `mint(..)` constraints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Error {
    /// A token account's mint is not the one `token(mint = ..)` names.
    Mint = 3100,
    /// A token account's owner is not the one `token(authority = ..)` names.
    Authority = 3101,
    /// A mint's decimals are not those `mint(decimals = ..)` gives.
    Decimals = 3102,
    /// A mint's authority is not the one `mint(authority = ..)` names.
    MintAuthority = 3103,
}

impl From<Error> for ProgramError {
    fn from(error: Error) -> Self {
        ProgramError::Custom(error as u32)
    }
}

/// `token(mint = .., authority = ..)` on a token account: each key given is
/// checked.
#[derive(Default)]
pub struct Check<'a> {
    pub mint: Option<&'a Entry>,
    pub authority: Option<&'a Entry>,
}

impl Check<'_> {
    pub fn check(&self, account: &impl Deref<Target = TokenAccountView>) -> Result {
        if self.mint.is_some_and(|mint| account.mint() != mint.key()) {
            return Err(Error::Mint.into());
        }
        if self.authority.is_some_and(|authority| account.owner() != authority.key()) {
            return Err(Error::Authority.into());
        }
        Ok(())
    }

    /// With the `debug` feature, logs which key `check` rejects for the
    /// account `name`, with what it found; how to fix it.
    #[cold]
    #[inline(never)]
    pub fn why(&self, account: &impl Deref<Target = TokenAccountView>, name: &str) -> &'static str {
        match (self.mint, self.authority) {
            (Some(mint), _) if account.mint() != mint.key() => {
                head("Mint", name)
                    .text("is a token account of mint ")
                    .key(account.mint())
                    .text(", expected mint ")
                    .key(mint.key())
                    .log();
                "pass the token account that holds the mint `token(mint = ..)` names, or pass that mint"
            }
            (_, Some(authority)) => {
                head("Authority", name)
                    .text("is a token account owned by ")
                    .key(account.owner())
                    .text(", expected its owner to be ")
                    .key(authority.key())
                    .log();
                "pass the token account that belongs to the account `token(authority = ..)` names"
            }
            _ => "",
        }
    }
}

/// `init(..), token(mint = .., authority = ..)`: both keys are required.
pub struct Init<'a> {
    pub mint: &'a Entry,
    pub authority: &'a Entry,
}

impl Init<'_> {
    /// The check `init` makes of the account each key names: the mint is one,
    /// or `InitializeAccount3` fails; an account that already exists is one
    /// the Token program made for a mint, and is checked to be this one's.
    pub const KEYS: &'static [(&'static str, cpi::Check)] = &[("mint", MINT_CHECK)];

    /// Creates the token account at `account`, which signs, under
    /// `token_program`, then initializes it for the mint and authority. With
    /// `idempotent`, an account that already is a token account is loaded
    /// instead, and checked.
    pub fn init<P: Wrapper, C: Callee, W: Wrapper + Deref<Target = TokenAccountView>>(
        &self,
        system: &Program<SystemProgram>,
        payer: &P,
        token_program: &C,
        mut account: Signer,
        idempotent: bool,
        rent: &RentSource,
    ) -> Result<W> {
        let check = Check { mint: Some(self.mint), authority: Some(self.authority) };
        if idempotent {
            match input::existing::<W, _>(account) {
                Ok(existing) => return check.check(&existing).map(|()| existing),
                Err(new) => account = new,
            }
        }
        const { is_token_program::<C>() };
        let (space, owner) = (size::<TokenAccount>(), *token_program.address());
        let signer = None::<&SignerSeeds<0>>;
        system::create(system, payer, &mut account, space, &owner, signer, rent)?;
        initialize_account3(token_program, &mut account, self.mint, self.authority.key())
            .invoke()?;
        // SAFETY: the Token program has just made it a token account of this
        // mint and authority.
        unsafe { input::made(account.into_entry()) }
    }
}

/// The `mint(..)` constraint, in its own module so a field reads
/// `mint(decimals = 6, authority = pool)`.
pub mod mint {
    use super::*;

    /// `mint(decimals = .., authority = ..)` on a mint: each key given is
    /// checked.
    #[derive(Default)]
    pub struct Check<'a> {
        pub decimals: Option<u8>,
        pub authority: Option<&'a Entry>,
    }

    impl Check<'_> {
        pub fn check(&self, mint: &impl Deref<Target = MintView>) -> Result {
            if self.decimals.is_some_and(|decimals| mint.decimals() != decimals) {
                return Err(Error::Decimals.into());
            }
            if self
                .authority
                .is_some_and(|authority| mint.mint_authority() != Some(authority.key()))
            {
                return Err(Error::MintAuthority.into());
            }
            Ok(())
        }

        /// As `token::Check::why`.
        #[cold]
        #[inline(never)]
        pub fn why(&self, mint: &impl Deref<Target = MintView>, name: &str) -> &'static str {
            match self.decimals {
                Some(decimals) if mint.decimals() != decimals => {
                    head("Decimals", name)
                        .text("is a mint with ")
                        .number(mint.decimals().into())
                        .text(" decimals, expected ")
                        .number(decimals.into())
                        .log();
                    "pass the mint `mint(decimals = ..)` describes"
                }
                _ => {
                    head("MintAuthority", name)
                        .text("is a mint whose authority is not the account `mint(authority = ..)` names")
                        .log();
                    "pass the mint that account may mint, or pass the mint's authority"
                }
            }
        }
    }

    /// `init(..), mint(decimals = .., authority = ..)`: both keys are required.
    pub struct Init<'a> {
        pub decimals: u8,
        pub authority: &'a Entry,
    }

    impl Init<'_> {
        /// Creates the mint at `account`, which signs, under `token_program`,
        /// then initializes it with the decimals and the authority, and no
        /// freeze authority. With `idempotent`, an account that already is a
        /// mint is loaded instead, and checked.
        pub fn init<P: Wrapper, C: Callee, W: Wrapper + Deref<Target = MintView>>(
            &self,
            system: &Program<SystemProgram>,
            payer: &P,
            token_program: &C,
            mut account: Signer,
            idempotent: bool,
            rent: &RentSource,
        ) -> Result<W> {
            let check = Check { decimals: Some(self.decimals), authority: Some(self.authority) };
            if idempotent {
                match input::existing::<W, _>(account) {
                    Ok(existing) => return check.check(&existing).map(|()| existing),
                    Err(new) => account = new,
                }
            }
            const { is_token_program::<C>() };
            let (space, owner) = (size::<Mint>(), *token_program.address());
            let signer = None::<&SignerSeeds<0>>;
            system::create(system, payer, &mut account, space, &owner, signer, rent)?;
            initialize_mint2(token_program, &mut account, self.decimals, self.authority.key())
                .invoke()?;
            // SAFETY: the Token program has just made it this mint.
            unsafe { input::made(account.into_entry()) }
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::{prelude::*, test_runner::RngSeed};
    use solana_program_pack::Pack;
    use spl_token_2022_interface::extension::StateWithExtensions;
    use std::{vec, vec::Vec};

    use super::*;

    /// Bytes that are often valid: tags, state and flag drawn from near the
    /// accepted values.
    fn near_valid(len: usize, bytes: Vec<u8>, picks: [u8; 4], at: [usize; 4]) -> Vec<u8> {
        let mut data = bytes;
        data.resize(len, 0);
        for (pick, at) in picks.into_iter().zip(at) {
            data[at..at + 4].copy_from_slice(&[pick % 3, 0, 0, 0]);
        }
        data
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 2048,
            rng_seed: RngSeed::Fixed(0x5350_4c54),
            ..ProptestConfig::default()
        })]

        #[test]
        fn a_token_account_is_valid_exactly_when_the_token_program_unpacks_it(
            bytes in prop::collection::vec(any::<u8>(), 165),
            picks in any::<[u8; 4]>(),
            state in 0u8..4,
        ) {
            let mut data = near_valid(165, bytes, picks, [72, 109, 129, 72]);
            data[108] = state;
            let theirs = spl_token_interface::state::Account::unpack(&data).is_ok();
            prop_assert_eq!(holds::<TokenAccount>(&data), theirs);
        }

        #[test]
        fn a_mint_is_valid_exactly_when_the_token_program_unpacks_it(
            bytes in prop::collection::vec(any::<u8>(), 82),
            picks in any::<[u8; 4]>(),
            initialized in 0u8..3,
        ) {
            let mut data = near_valid(82, bytes, picks, [0, 46, 0, 46]);
            data[45] = initialized;
            let theirs = spl_token_interface::state::Mint::unpack(&data).is_ok();
            prop_assert_eq!(holds::<Mint>(&data), theirs);
        }
    }

    /// Whether `data` is exactly a `K` its program would unpack.
    fn holds<K: Kind>(data: &[u8]) -> bool {
        K::SIZE == Some(data.len()) && K::view(data).is_ok_and(K::validate)
    }

    /// Base-valid bytes of `len`, with Token-2022's padding and account type
    /// often, but not always, as it requires.
    fn extended(
        base: Vec<u8>,
        len: usize,
        tail: Vec<u8>,
        account_type: u8,
        zeroed: bool,
    ) -> Vec<u8> {
        let base_len = base.len();
        let mut data = base;
        data.extend(tail);
        data.resize(len, 0);
        if len > 165 {
            if zeroed {
                data[base_len..165].fill(0);
            }
            data[165] = account_type % 4;
        }
        data
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 2048,
            rng_seed: RngSeed::Fixed(0x3230_3232),
            ..ProptestConfig::default()
        })]

        #[test]
        fn a_token_2022_account_is_valid_exactly_when_token_2022_unpacks_it(
            len in prop::sample::select(vec![165, 166, 170, 355, 400]),
            tail in prop::collection::vec(any::<u8>(), 0..240),
            account_type in 0u8..4,
        ) {
            let base = token_account_bytes();
            let data = extended(base, len, tail, account_type, true);
            let theirs = StateWithExtensions::<spl_token_2022_interface::state::Account>::unpack(&data).is_ok();
            prop_assert_eq!(<TokenAccount as InterfaceKind>::validate(&data), theirs);
        }

        #[test]
        fn a_token_2022_mint_is_valid_exactly_when_token_2022_unpacks_it(
            len in prop::sample::select(vec![82, 165, 166, 170, 355, 400]),
            tail in prop::collection::vec(any::<u8>(), 0..320),
            account_type in 0u8..4,
            zeroed in any::<bool>(),
        ) {
            let base = mint_bytes();
            let data = extended(base, len, tail, account_type, zeroed);
            let theirs = StateWithExtensions::<spl_token_2022_interface::state::Mint>::unpack(&data).is_ok();
            prop_assert_eq!(<Mint as InterfaceKind>::validate(&data), theirs);
        }
    }

    fn token_account_bytes() -> Vec<u8> {
        let mut data = vec![0; 165];
        data[108] = 1; // initialized
        data
    }

    fn mint_bytes() -> Vec<u8> {
        let mut data = vec![0; 82];
        data[45] = 1; // initialized
        data
    }

    #[test]
    fn the_layouts_have_the_token_programs_lengths() {
        assert_eq!(size::<TokenAccount>(), spl_token_interface::state::Account::LEN);
        assert_eq!(size::<Mint>(), spl_token_interface::state::Mint::LEN);
    }
}
