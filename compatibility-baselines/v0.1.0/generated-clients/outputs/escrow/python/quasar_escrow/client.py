"""Generated client for the quasar_escrow program."""
from __future__ import annotations

import struct
from dataclasses import dataclass
from typing import Optional

from solders.pubkey import Pubkey
from solders.instruction import Instruction, AccountMeta

class DecodeError(ValueError):
    pass

_MAX_DECODE_ELEMENTS = 10 * 1024 * 1024

def _take(data: bytes, offset: int, size: int) -> tuple[bytes, int]:
    if size < 0 or offset < 0 or size > len(data) - offset:
        raise DecodeError("truncated input")
    end = offset + size
    return data[offset:end], end

def _unpack(fmt: str, data: bytes, offset: int) -> tuple[object, int]:
    raw, offset = _take(data, offset, struct.calcsize(fmt))
    return struct.unpack(fmt, raw)[0], offset

def _finish(data: bytes, offset: int) -> None:
    if offset != len(data):
        raise DecodeError("trailing bytes")

PROGRAM_ID = Pubkey.from_string("22222222222222222222222222222222222222222222")

MAKE_DISCRIMINATOR = bytes([0])
TAKE_DISCRIMINATOR = bytes([1])
REFUND_DISCRIMINATOR = bytes([2])

ESCROW_ACCOUNT_DISCRIMINATOR = bytes([1])

MAKE_EVENT_EVENT_DISCRIMINATOR = bytes([1])
REFUND_EVENT_EVENT_DISCRIMINATOR = bytes([3])
TAKE_EVENT_EVENT_DISCRIMINATOR = bytes([2])


@dataclass
class Escrow:
    maker: Pubkey
    mint_a: Pubkey
    mint_b: Pubkey
    maker_ta_b: Pubkey
    receive: int
    bump: int

    @classmethod
    def decode(cls, data: bytes) -> Escrow:
        offset = 0
        _raw, offset = _take(data, offset, 32)
        maker = Pubkey.from_bytes(_raw)
        _raw, offset = _take(data, offset, 32)
        mint_a = Pubkey.from_bytes(_raw)
        _raw, offset = _take(data, offset, 32)
        mint_b = Pubkey.from_bytes(_raw)
        _raw, offset = _take(data, offset, 32)
        maker_ta_b = Pubkey.from_bytes(_raw)
        receive, offset = _unpack("<Q", data, offset)
        bump, offset = _unpack("<B", data, offset)
        _finish(data, offset)
        return cls(maker=maker, mint_a=mint_a, mint_b=mint_b, maker_ta_b=maker_ta_b, receive=receive, bump=bump)


@dataclass
class MakeEvent:
    escrow: Pubkey
    maker: Pubkey
    mint_a: Pubkey
    mint_b: Pubkey
    deposit: int
    receive: int

    @classmethod
    def decode(cls, data: bytes) -> MakeEvent:
        offset = 0
        _raw, offset = _take(data, offset, 32)
        escrow = Pubkey.from_bytes(_raw)
        _raw, offset = _take(data, offset, 32)
        maker = Pubkey.from_bytes(_raw)
        _raw, offset = _take(data, offset, 32)
        mint_a = Pubkey.from_bytes(_raw)
        _raw, offset = _take(data, offset, 32)
        mint_b = Pubkey.from_bytes(_raw)
        deposit, offset = _unpack("<Q", data, offset)
        receive, offset = _unpack("<Q", data, offset)
        _finish(data, offset)
        return cls(escrow=escrow, maker=maker, mint_a=mint_a, mint_b=mint_b, deposit=deposit, receive=receive)


@dataclass
class RefundEvent:
    escrow: Pubkey

    @classmethod
    def decode(cls, data: bytes) -> RefundEvent:
        offset = 0
        _raw, offset = _take(data, offset, 32)
        escrow = Pubkey.from_bytes(_raw)
        _finish(data, offset)
        return cls(escrow=escrow)


@dataclass
class TakeEvent:
    escrow: Pubkey

    @classmethod
    def decode(cls, data: bytes) -> TakeEvent:
        offset = 0
        _raw, offset = _take(data, offset, 32)
        escrow = Pubkey.from_bytes(_raw)
        _finish(data, offset)
        return cls(escrow=escrow)


@dataclass
class MakeInput:
    maker: Pubkey
    mint_a: Pubkey
    mint_b: Pubkey
    maker_ta_a: Pubkey
    maker_ta_b: Pubkey
    vault_ta_a: Pubkey
    deposit: int
    receive: int


def create_make_instruction(input: MakeInput) -> Instruction:
    accounts_map = {}
    accounts = []
    accounts_map["maker"] = input.maker
    accounts.append(AccountMeta(accounts_map["maker"], is_signer=True, is_writable=True))
    accounts_map["escrow"] = Pubkey.find_program_address([bytes([101, 115, 99, 114, 111, 119]), bytes(accounts_map["maker"])], PROGRAM_ID)[0]
    accounts.append(AccountMeta(accounts_map["escrow"], is_signer=False, is_writable=True))
    accounts_map["mintA"] = input.mint_a
    accounts.append(AccountMeta(accounts_map["mintA"], is_signer=False, is_writable=False))
    accounts_map["mintB"] = input.mint_b
    accounts.append(AccountMeta(accounts_map["mintB"], is_signer=False, is_writable=False))
    accounts_map["makerTaA"] = input.maker_ta_a
    accounts.append(AccountMeta(accounts_map["makerTaA"], is_signer=False, is_writable=True))
    accounts_map["makerTaB"] = input.maker_ta_b
    accounts.append(AccountMeta(accounts_map["makerTaB"], is_signer=True, is_writable=True))
    accounts_map["vaultTaA"] = input.vault_ta_a
    accounts.append(AccountMeta(accounts_map["vaultTaA"], is_signer=True, is_writable=True))
    accounts_map["rent"] = Pubkey.from_string("SysvarRent111111111111111111111111111111111")
    accounts.append(AccountMeta(accounts_map["rent"], is_signer=False, is_writable=False))
    accounts_map["tokenProgram"] = Pubkey.from_string("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
    accounts.append(AccountMeta(accounts_map["tokenProgram"], is_signer=False, is_writable=False))
    accounts_map["systemProgram"] = Pubkey.from_string("11111111111111111111111111111111")
    accounts.append(AccountMeta(accounts_map["systemProgram"], is_signer=False, is_writable=False))
    data = bytearray(MAKE_DISCRIMINATOR)
    data += struct.pack("<Q", input.deposit)
    data += struct.pack("<Q", input.receive)
    data = bytes(data)
    return Instruction(PROGRAM_ID, data, accounts)


@dataclass
class TakeInput:
    taker: Pubkey
    maker: Pubkey
    mint_a: Pubkey
    mint_b: Pubkey
    taker_ta_a: Pubkey
    taker_ta_b: Pubkey
    maker_ta_b: Pubkey
    vault_ta_a: Pubkey


def create_take_instruction(input: TakeInput) -> Instruction:
    accounts_map = {}
    accounts = []
    accounts_map["taker"] = input.taker
    accounts.append(AccountMeta(accounts_map["taker"], is_signer=True, is_writable=True))
    accounts_map["escrow"] = Pubkey.find_program_address([bytes([101, 115, 99, 114, 111, 119]), bytes(accounts_map["maker"])], PROGRAM_ID)[0]
    accounts.append(AccountMeta(accounts_map["escrow"], is_signer=False, is_writable=True))
    accounts_map["maker"] = input.maker
    accounts.append(AccountMeta(accounts_map["maker"], is_signer=False, is_writable=True))
    accounts_map["mintA"] = input.mint_a
    accounts.append(AccountMeta(accounts_map["mintA"], is_signer=False, is_writable=False))
    accounts_map["mintB"] = input.mint_b
    accounts.append(AccountMeta(accounts_map["mintB"], is_signer=False, is_writable=False))
    accounts_map["takerTaA"] = input.taker_ta_a
    accounts.append(AccountMeta(accounts_map["takerTaA"], is_signer=True, is_writable=True))
    accounts_map["takerTaB"] = input.taker_ta_b
    accounts.append(AccountMeta(accounts_map["takerTaB"], is_signer=False, is_writable=True))
    accounts_map["makerTaB"] = input.maker_ta_b
    accounts.append(AccountMeta(accounts_map["makerTaB"], is_signer=True, is_writable=True))
    accounts_map["vaultTaA"] = input.vault_ta_a
    accounts.append(AccountMeta(accounts_map["vaultTaA"], is_signer=False, is_writable=True))
    accounts_map["rent"] = Pubkey.from_string("SysvarRent111111111111111111111111111111111")
    accounts.append(AccountMeta(accounts_map["rent"], is_signer=False, is_writable=False))
    accounts_map["tokenProgram"] = Pubkey.from_string("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
    accounts.append(AccountMeta(accounts_map["tokenProgram"], is_signer=False, is_writable=False))
    accounts_map["systemProgram"] = Pubkey.from_string("11111111111111111111111111111111")
    accounts.append(AccountMeta(accounts_map["systemProgram"], is_signer=False, is_writable=False))
    data = TAKE_DISCRIMINATOR
    return Instruction(PROGRAM_ID, data, accounts)


@dataclass
class RefundInput:
    maker: Pubkey
    mint_a: Pubkey
    maker_ta_a: Pubkey
    vault_ta_a: Pubkey


def create_refund_instruction(input: RefundInput) -> Instruction:
    accounts_map = {}
    accounts = []
    accounts_map["maker"] = input.maker
    accounts.append(AccountMeta(accounts_map["maker"], is_signer=True, is_writable=True))
    accounts_map["escrow"] = Pubkey.find_program_address([bytes([101, 115, 99, 114, 111, 119]), bytes(accounts_map["maker"])], PROGRAM_ID)[0]
    accounts.append(AccountMeta(accounts_map["escrow"], is_signer=False, is_writable=True))
    accounts_map["mintA"] = input.mint_a
    accounts.append(AccountMeta(accounts_map["mintA"], is_signer=False, is_writable=False))
    accounts_map["makerTaA"] = input.maker_ta_a
    accounts.append(AccountMeta(accounts_map["makerTaA"], is_signer=True, is_writable=True))
    accounts_map["vaultTaA"] = input.vault_ta_a
    accounts.append(AccountMeta(accounts_map["vaultTaA"], is_signer=False, is_writable=True))
    accounts_map["rent"] = Pubkey.from_string("SysvarRent111111111111111111111111111111111")
    accounts.append(AccountMeta(accounts_map["rent"], is_signer=False, is_writable=False))
    accounts_map["tokenProgram"] = Pubkey.from_string("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
    accounts.append(AccountMeta(accounts_map["tokenProgram"], is_signer=False, is_writable=False))
    accounts_map["systemProgram"] = Pubkey.from_string("11111111111111111111111111111111")
    accounts.append(AccountMeta(accounts_map["systemProgram"], is_signer=False, is_writable=False))
    data = REFUND_DISCRIMINATOR
    return Instruction(PROGRAM_ID, data, accounts)


def decode_event(data: bytes) -> Optional[tuple[str, object]]:
    """Decode an event from raw log data. Returns (event_name, event_data) or None."""
    if data[:1] == MAKE_EVENT_EVENT_DISCRIMINATOR:
        return ("MakeEvent", MakeEvent.decode(data[1:]))
    if data[:1] == REFUND_EVENT_EVENT_DISCRIMINATOR:
        return ("RefundEvent", RefundEvent.decode(data[1:]))
    if data[:1] == TAKE_EVENT_EVENT_DISCRIMINATOR:
        return ("TakeEvent", TakeEvent.decode(data[1:]))
    return None


class QuasarEscrowClient:
    program_id = PROGRAM_ID

    @staticmethod
    def make(input: MakeInput) -> Instruction:
        return create_make_instruction(input)

    @staticmethod
    def take(input: TakeInput) -> Instruction:
        return create_take_instruction(input)

    @staticmethod
    def refund(input: RefundInput) -> Instruction:
        return create_refund_instruction(input)

    @staticmethod
    def decode_event(data: bytes) -> Optional[tuple[str, object]]:
        return decode_event(data)
