use {quasar_lang::cpi::CpiDynamic, solana_address::Address};

fn main() {
    let program_id = Address::new_from_array([0u8; 32]);
    let mut cpi = CpiDynamic::<0, 4>::new(&program_id);

    cpi.set_data_len(4).unwrap();
}
