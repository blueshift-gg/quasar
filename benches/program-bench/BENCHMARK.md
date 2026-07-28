## Quasar CU Benchmark

Compute units per instruction. Δ is the change against the previous run of this table.

| Instruction | CU | Δ |
| --- | --- | --- |
| ping | 21 | 0 |
| log | 127 | 0 |
| create_account | 1478 | 0 |
| transfer | 1267 | 0 |
| unchecked_accounts | 108 | 0 |
| accounts | 876 | 0 |
| create_pda_static | 2107 | 0 |
| verify_pda_static | 249 | 0 |
| create_pda_dynamic | 2801 | 0 |
| verify_pda_dynamic | 282 | 0 |
| dynamic_readback | 174 | 0 |
| dynamic_mutate | 890 | 0 |
| dynamic_view_mut | 2102 | 0 |
| two_dyn | 422 | 0 |
| dyn_bytes_check | 93 | 0 |
| dyn_str_check | 199 | 0 |
| create_account_cpi | 1269 | 0 |
| close_account | 329 | 0 |
| realloc_check | 1473 | 0 |
| assign_test | 1233 | 0 |
| space_override | 1842 | 0 |
| init_mint | 5317 | 0 |
| init_token_account | 6769 | 0 |
| init_ata | 23153 | 0 |
| transfer_checked | 7477 | 0 |
| mint_to | 5716 | 0 |
| burn | 5930 | 0 |
| approve | 4085 | 0 |
| revoke | 3791 | 0 |
| close_token_account | 4098 | 0 |
| sweep_and_close | 11592 | 0 |
| init_mint_t22 | 3566 | 0 |
| transfer_checked_interface | 7481 | 0 |
| header_nodup_signer | 26 | 0 |
| header_dup_signer | 37 | 0 |
| two_accounts_check | 110 | 0 |
| has_one_default | 108 | 0 |
| owner_check | 83 | 0 |
| program_check | 34 | 0 |
| require_eq_check | 33 | 0 |
| remaining_typed_check | 77 | 0 |
| emit_empty_event | 238 | 0 |
| emit_u64_event | 278 | 0 |
| emit_multi_field | 329 | 0 |
| emit_large_event | 430 | 0 |
| emit_two_events | 499 | 0 |
| emit_via_cpi | 1364 | 0 |
| heap_vec_ok | 79 | 0 |
| read_clock | 1917 | 0 |
| read_clock_from_account | 120 | 0 |
| read_rent | 1930 | 0 |
| read_rent_calc | 1940 | 0 |
| return_u64 | 180 | 0 |
| cpi_invoke_with_return | 1474 | 0 |
| cpi_invoke_ignore_return | 1242 | 0 |
| cpi_mut_readback | 1319 | 0 |

*Binary size*: 135904 bytes. *Commit*: `a636c770-dirty`.
