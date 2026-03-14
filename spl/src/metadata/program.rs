use {
    crate::metadata::constants::METADATA_PROGRAM_BYTES,
    quasar_core::{prelude::*, traits::Id},
};

quasar_core::define_account!(pub struct MetadataProgram => [checks::Executable, checks::Address]);

impl Id for MetadataProgram {
    const ID: Address = Address::new_from_array(METADATA_PROGRAM_BYTES);
}
