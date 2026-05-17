const LDDW: u8 = 0x18;

pub(crate) struct InstructionWalker<'a> {
    text: &'a [u8],
    text_base: u64,
    offset: usize,
}

impl<'a> InstructionWalker<'a> {
    pub(crate) fn new(text: &'a [u8], text_base: u64) -> Self {
        Self {
            text,
            text_base,
            offset: 0,
        }
    }
}

impl Iterator for InstructionWalker<'_> {
    /// (virtual address, opcode)
    type Item = (u64, u8);

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.text.len() {
            return None;
        }

        let addr = self.text_base + self.offset as u64;
        let opcode = self.text[self.offset];

        if opcode == LDDW {
            // lddw occupies 2 slots (16 bytes) but counts as 1 instruction
            self.offset += 16;
        } else {
            self.offset += 8;
        }

        Some((addr, opcode))
    }
}
