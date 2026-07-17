"""Generated client for the quasar_multisig program."""
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

PROGRAM_ID = Pubkey.from_string("44444444444444444444444444444444444444444444")

CREATE_DISCRIMINATOR = bytes([0])
DEPOSIT_DISCRIMINATOR = bytes([1])
SET_LABEL_DISCRIMINATOR = bytes([2])
EXECUTE_TRANSFER_DISCRIMINATOR = bytes([3])

MULTISIG_CONFIG_ACCOUNT_DISCRIMINATOR = bytes([1])


@dataclass
class MultisigConfig:
    creator: Pubkey
    threshold: int
    bump: int
    label: str
    signers: list[Pubkey]

    @classmethod
    def decode(cls, data: bytes) -> MultisigConfig:
        offset = 0
        _raw, offset = _take(data, offset, 32)
        creator = Pubkey.from_bytes(_raw)
        threshold, offset = _unpack("<B", data, offset)
        bump, offset = _unpack("<B", data, offset)
        label_len, offset = _unpack("<B", data, offset)
        signers_len, offset = _unpack("<H", data, offset)
        _raw, offset = _take(data, offset, label_len)
        try:
            label = _raw.decode("utf-8")
        except UnicodeDecodeError as exc:
            raise DecodeError("invalid UTF-8") from exc
        if signers_len > _MAX_DECODE_ELEMENTS or signers_len > len(data) - offset:
            raise DecodeError("element count exceeds limit")
        signers = []
        for _ in range(signers_len):
            _raw, offset = _take(data, offset, 32)
            _item = Pubkey.from_bytes(_raw)
            signers.append(_item)
        _finish(data, offset)
        return cls(creator=creator, threshold=threshold, bump=bump, label=label, signers=signers)


@dataclass
class CreateInput:
    creator: Pubkey
    threshold: int
    remaining_accounts: list[AccountMeta] = None


def create_create_instruction(input: CreateInput) -> Instruction:
    accounts_map = {}
    accounts = []
    accounts_map["creator"] = input.creator
    accounts_map["rent"] = Pubkey.from_string("SysvarRent111111111111111111111111111111111")
    accounts_map["systemProgram"] = Pubkey.from_string("11111111111111111111111111111111")
    accounts_map["config"] = Pubkey.find_program_address([bytes([109, 117, 108, 116, 105, 115, 105, 103]), bytes(accounts_map["creator"])], PROGRAM_ID)[0]
    accounts.append(AccountMeta(accounts_map["creator"], is_signer=True, is_writable=True))
    accounts.append(AccountMeta(accounts_map["config"], is_signer=False, is_writable=True))
    accounts.append(AccountMeta(accounts_map["rent"], is_signer=False, is_writable=False))
    accounts.append(AccountMeta(accounts_map["systemProgram"], is_signer=False, is_writable=False))
    if input.remaining_accounts:
        accounts.extend(input.remaining_accounts)
    data = bytearray(CREATE_DISCRIMINATOR)
    data += struct.pack("<B", input.threshold)
    data = bytes(data)
    return Instruction(PROGRAM_ID, data, accounts)


@dataclass
class DepositInput:
    depositor: Pubkey
    config: Pubkey
    amount: int


def create_deposit_instruction(input: DepositInput) -> Instruction:
    accounts_map = {}
    accounts = []
    accounts_map["depositor"] = input.depositor
    accounts_map["config"] = input.config
    accounts_map["systemProgram"] = Pubkey.from_string("11111111111111111111111111111111")
    accounts_map["vault"] = Pubkey.find_program_address([bytes([118, 97, 117, 108, 116]), bytes(accounts_map["config"])], PROGRAM_ID)[0]
    accounts.append(AccountMeta(accounts_map["depositor"], is_signer=True, is_writable=True))
    accounts.append(AccountMeta(accounts_map["config"], is_signer=False, is_writable=False))
    accounts.append(AccountMeta(accounts_map["vault"], is_signer=False, is_writable=True))
    accounts.append(AccountMeta(accounts_map["systemProgram"], is_signer=False, is_writable=False))
    data = bytearray(DEPOSIT_DISCRIMINATOR)
    data += struct.pack("<Q", input.amount)
    data = bytes(data)
    return Instruction(PROGRAM_ID, data, accounts)


@dataclass
class SetLabelInput:
    creator: Pubkey
    label: str


def create_set_label_instruction(input: SetLabelInput) -> Instruction:
    accounts_map = {}
    accounts = []
    accounts_map["creator"] = input.creator
    accounts_map["systemProgram"] = Pubkey.from_string("11111111111111111111111111111111")
    accounts_map["config"] = Pubkey.find_program_address([bytes([109, 117, 108, 116, 105, 115, 105, 103]), bytes(accounts_map["creator"])], PROGRAM_ID)[0]
    accounts.append(AccountMeta(accounts_map["creator"], is_signer=True, is_writable=True))
    accounts.append(AccountMeta(accounts_map["config"], is_signer=False, is_writable=True))
    accounts.append(AccountMeta(accounts_map["systemProgram"], is_signer=False, is_writable=False))
    data = bytearray(SET_LABEL_DISCRIMINATOR)
    _label_b = input.label.encode("utf-8")
    data += struct.pack("<B", len(_label_b))
    data += _label_b
    data = bytes(data)
    return Instruction(PROGRAM_ID, data, accounts)


@dataclass
class ExecuteTransferInput:
    creator: Pubkey
    recipient: Pubkey
    amount: int
    remaining_accounts: list[AccountMeta] = None


def create_execute_transfer_instruction(input: ExecuteTransferInput) -> Instruction:
    accounts_map = {}
    accounts = []
    accounts_map["creator"] = input.creator
    accounts_map["recipient"] = input.recipient
    accounts_map["systemProgram"] = Pubkey.from_string("11111111111111111111111111111111")
    accounts_map["config"] = Pubkey.find_program_address([bytes([109, 117, 108, 116, 105, 115, 105, 103]), bytes(accounts_map["creator"])], PROGRAM_ID)[0]
    accounts_map["vault"] = Pubkey.find_program_address([bytes([118, 97, 117, 108, 116]), bytes(accounts_map["config"])], PROGRAM_ID)[0]
    accounts.append(AccountMeta(accounts_map["config"], is_signer=False, is_writable=False))
    accounts.append(AccountMeta(accounts_map["creator"], is_signer=False, is_writable=False))
    accounts.append(AccountMeta(accounts_map["vault"], is_signer=False, is_writable=True))
    accounts.append(AccountMeta(accounts_map["recipient"], is_signer=False, is_writable=True))
    accounts.append(AccountMeta(accounts_map["systemProgram"], is_signer=False, is_writable=False))
    if input.remaining_accounts:
        accounts.extend(input.remaining_accounts)
    data = bytearray(EXECUTE_TRANSFER_DISCRIMINATOR)
    data += struct.pack("<Q", input.amount)
    data = bytes(data)
    return Instruction(PROGRAM_ID, data, accounts)


class QuasarMultisigClient:
    program_id = PROGRAM_ID

    @staticmethod
    def create(input: CreateInput) -> Instruction:
        return create_create_instruction(input)

    @staticmethod
    def deposit(input: DepositInput) -> Instruction:
        return create_deposit_instruction(input)

    @staticmethod
    def set_label(input: SetLabelInput) -> Instruction:
        return create_set_label_instruction(input)

    @staticmethod
    def execute_transfer(input: ExecuteTransferInput) -> Instruction:
        return create_execute_transfer_instruction(input)
