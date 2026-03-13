use {
    crate::{dwarf::Resolver, elf::ElfInfo, walk::InstructionWalker},
    std::collections::HashMap,
};

pub struct ProfileResult {
    pub folded_stacks: String,
    pub total_cus: u64,
    /// (function_name, self_cu_count) sorted descending
    pub function_cus: Vec<(String, u64)>,
}

pub fn profile(mmap: &[u8], info: &ElfInfo, resolver: &Resolver) -> ProfileResult {
    let text = &mmap[info.text_offset..info.text_offset + info.text_size];
    let walker = InstructionWalker::new(text, info.text_base_addr);

    let mut stack_counts: HashMap<Vec<String>, u64> = HashMap::new();
    let mut leaf_counts: HashMap<String, u64> = HashMap::new();
    let mut total_cus: u64 = 0;

    for (addr, _opcode) in walker {
        let mut stack = resolver.resolve(addr);
        total_cus += 1;

        // Attribute to leaf function (innermost frame)
        // addr2line returns frames innermost-first, so first() is the leaf
        if let Some(leaf) = stack.first() {
            *leaf_counts.entry(leaf.clone()).or_insert(0) += 1;
        }

        // Reverse to outermost-first order for folded stack format
        stack.reverse();
        *stack_counts.entry(stack).or_insert(0) += 1;
    }

    // Build folded stacks string, sorted for determinism
    let mut entries: Vec<_> = stack_counts.into_iter().collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut folded = String::new();
    for (stack, count) in &entries {
        folded.push_str(&stack.join(";"));
        folded.push(' ');
        folded.push_str(&count.to_string());
        folded.push('\n');
    }

    // Build sorted function CU table
    let mut function_cus: Vec<_> = leaf_counts.into_iter().collect();
    function_cus.sort_by_key(|b| std::cmp::Reverse(b.1));

    ProfileResult {
        folded_stacks: folded,
        total_cus,
        function_cus,
    }
}
