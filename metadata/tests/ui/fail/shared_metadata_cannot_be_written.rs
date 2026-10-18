use quasar_lang::prelude::*;
use quasar_metadata::{MetadataAccount, MetadataProgram, instructions};

fn write(program: &Program<MetadataProgram>, metadata: &MetadataAccount, creator: &Signer) {
    let _call = instructions::sign_metadata(program, metadata, creator);
}

fn main() {}
