//! Static compute-unit aggregation for the CLI profiler.

use {
    super::{dwarf::Resolver, elf::ElfInfo, walk::InstructionWalker},
    std::collections::HashMap,
};

pub(crate) struct ProfileResult {
    pub(crate) stack_counts: Vec<(Vec<String>, u64)>,
    pub(crate) total_cus: u64,
    /// (function_name, self_cu_count) sorted descending
    pub(crate) function_cus: Vec<(String, u64)>,
}

pub(crate) fn profile(mmap: &[u8], info: &ElfInfo, resolver: &Resolver) -> ProfileResult {
    let text = &mmap[info.text_offset..info.text_offset + info.text_size];
    let walker = InstructionWalker::new(text, info.text_base_addr);

    let mut stack_counts: HashMap<Vec<String>, u64> = HashMap::new();
    let mut leaf_counts: HashMap<String, u64> = HashMap::new();
    let mut total_cus: u64 = 0;

    for (addr, _opcode) in walker {
        let stack = resolver.resolve(addr);
        total_cus += 1;

        // addr2line returns frames innermost-first.
        if let Some(leaf) = stack.first() {
            *leaf_counts.entry(leaf.clone()).or_insert(0) += 1;
        }

        *stack_counts.entry(stack).or_insert(0) += 1;
    }

    let mut function_cus: Vec<_> = leaf_counts.into_iter().collect();
    function_cus.sort_by_key(|b| std::cmp::Reverse(b.1));

    let mut stack_counts: Vec<_> = stack_counts.into_iter().collect();
    stack_counts.sort_by(|a, b| a.0.iter().cmp(b.0.iter()));

    ProfileResult {
        stack_counts,
        total_cus,
        function_cus,
    }
}
