use super::*;

#[test]
fn dynamic_size_sweep() {
    let name_lens: &[usize] = &[0, 1, 7, 8, 15, 16, 31, 32];
    let tag_counts: &[usize] = &[0, 1, 5, 10];

    for &name_len in name_lens {
        for &tags_count in tag_counts {
            let name = vec![b'x'; name_len];
            let tags: Vec<[u8; 32]> = (0..tags_count).map(|i| [i as u8; 32]).collect();
            let mut buf = make_dyn_buffer_exact(&name, &tags);
            let view = unsafe { buf.view() };
            let data = unsafe { view.borrow_unchecked() };

            let expected_len = DYN_HEADER_SIZE + 4 + name_len + 4 + tags_count * 32;
            assert_eq!(data.len(), expected_len);

            let mut offset = DYN_HEADER_SIZE;
            let read_name_len =
                u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
            assert_eq!(read_name_len, name_len);
            offset += 4 + name_len;

            let read_tags_count =
                u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
            assert_eq!(read_tags_count, tags_count);
            offset += 4;

            if tags_count > 0 {
                let slice: &[Address] = unsafe {
                    core::slice::from_raw_parts(
                        data[offset..].as_ptr() as *const Address,
                        tags_count,
                    )
                };
                assert_eq!(
                    slice[tags_count - 1].as_array(),
                    &[(tags_count - 1) as u8; 32]
                );
            }
        }
    }
}

#[test]
fn dynamic_memmove_1byte_grow_1byte_tail() {
    let name_data_offset = DYN_HEADER_SIZE + 4;
    let data_len = name_data_offset + 1 + 1;
    let mut buf = AccountBuffer::new(data_len);
    buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[0] = 0x05;
    data[DYN_DISC_LEN..DYN_DISC_LEN + 32].copy_from_slice(&[0xAA; 32]);
    data[DYN_HEADER_SIZE..DYN_HEADER_SIZE + 4].copy_from_slice(&1u32.to_le_bytes());
    data[name_data_offset] = b'A';
    data[name_data_offset + 1] = 0xEE;
    buf.write_data(&data);

    let mut view = unsafe { buf.view() };
    resize(&mut view, data_len + 1).unwrap();
    let data = unsafe { view.borrow_unchecked_mut() };

    let old_end = name_data_offset + 1;
    let new_end = name_data_offset + 2;
    unsafe {
        core::ptr::copy(
            data.as_ptr().add(old_end),
            data.as_mut_ptr().add(new_end),
            1,
        );
    }
    data[name_data_offset] = b'A';
    data[name_data_offset + 1] = b'B';
    assert_eq!(data[new_end], 0xEE);
}

#[test]
fn dynamic_memmove_1byte_shrink_overlapping() {
    let name_data_offset = DYN_HEADER_SIZE + 4;
    let data_len = name_data_offset + 2 + 2;
    let mut buf = AccountBuffer::new(data_len);
    buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[0] = 0x05;
    data[DYN_DISC_LEN..DYN_DISC_LEN + 32].copy_from_slice(&[0xAA; 32]);
    data[DYN_HEADER_SIZE..DYN_HEADER_SIZE + 4].copy_from_slice(&2u32.to_le_bytes());
    data[name_data_offset] = b'A';
    data[name_data_offset + 1] = b'B';
    data[name_data_offset + 2] = 0xDD;
    data[name_data_offset + 3] = 0xEE;
    buf.write_data(&data);

    let mut view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked_mut() };

    let old_end = name_data_offset + 2;
    let new_end = name_data_offset + 1;
    unsafe {
        core::ptr::copy(
            data.as_ptr().add(old_end),
            data.as_mut_ptr().add(new_end),
            2,
        );
    }
    data[name_data_offset] = b'A';
    assert_eq!(data[name_data_offset + 1], 0xDD);
    assert_eq!(data[name_data_offset + 2], 0xEE);
    resize(&mut view, name_data_offset + 3).unwrap();
}

#[test]
fn dynamic_batch_write_shared_read_then_mut_write() {
    let name = b"hello";
    let tags = [[0xDD; 32]];
    let mut buf = make_dyn_buffer_exact(name, &tags);
    let mut view = unsafe { buf.view() };

    let mut preserved_tag = [0u8; 32];
    {
        let data = unsafe { view.borrow_unchecked() };
        let mut offset = DYN_HEADER_SIZE;
        let name_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4 + name_len;
        let tags_count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;
        assert_eq!(tags_count, 1);
        preserved_tag.copy_from_slice(&data[offset..offset + 32]);
    }

    let new_name = b"hi";
    let new_total = DYN_HEADER_SIZE + 4 + new_name.len() + 4 + 32;
    {
        let data = unsafe { view.borrow_unchecked_mut() };
        let mut offset = DYN_HEADER_SIZE;
        data[offset..offset + 4].copy_from_slice(&(new_name.len() as u32).to_le_bytes());
        offset += 4;
        data[offset..offset + new_name.len()].copy_from_slice(new_name);
        offset += new_name.len();
        data[offset..offset + 4].copy_from_slice(&1u32.to_le_bytes());
        offset += 4;
        data[offset..offset + 32].copy_from_slice(&preserved_tag);
    }
    resize(&mut view, new_total).unwrap();

    let data = unsafe { view.borrow_unchecked() };
    let mut offset = DYN_HEADER_SIZE;
    let name_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    assert_eq!(name_len, 2);
    let s = unsafe { core::str::from_utf8_unchecked(&data[offset..offset + name_len]) };
    assert_eq!(s, "hi");
    offset += name_len;
    let tags_count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    assert_eq!(tags_count, 1);
    assert_eq!(&data[offset..offset + 32], &[0xDD; 32]);
}

#[test]
fn dynamic_vec_mut_write_then_shared_read() {
    let tags = [[0x11; 32]];
    let mut buf = make_dyn_buffer_exact(b"", &tags);
    let mut view = unsafe { buf.view() };

    let tags_data_offset = DYN_HEADER_SIZE + 4 + 4;

    {
        let data = unsafe { view.borrow_unchecked_mut() };
        let slice: &mut [Address] = unsafe {
            core::slice::from_raw_parts_mut(
                data[tags_data_offset..].as_mut_ptr() as *mut Address,
                1,
            )
        };
        slice[0] = Address::new_from_array([0xFF; 32]);
    }

    {
        let data = unsafe { view.borrow_unchecked() };
        let slice: &[Address] = unsafe {
            core::slice::from_raw_parts(data[tags_data_offset..].as_ptr() as *const Address, 1)
        };
        assert_eq!(slice[0].as_array(), &[0xFF; 32]);
    }
}

#[test]
fn dynamic_copy_nonoverlapping_at_allocation_edge() {
    let mut buf = make_dyn_buffer_exact(b"", &[]);
    let mut view = unsafe { buf.view() };
    let target_len = DYN_HEADER_SIZE + 4 + 4 + 96;
    resize(&mut view, target_len).unwrap();

    let new_tags = [
        Address::new_from_array([0xAA; 32]),
        Address::new_from_array([0xBB; 32]),
        Address::new_from_array([0xCC; 32]),
    ];

    let data = unsafe { view.borrow_unchecked_mut() };
    let tags_data_offset = DYN_HEADER_SIZE + 4 + 4;
    let tags_prefix_offset = DYN_HEADER_SIZE + 4;
    data[tags_prefix_offset..tags_prefix_offset + 4].copy_from_slice(&3u32.to_le_bytes());

    assert_eq!(tags_data_offset + 96, target_len);
    unsafe {
        core::ptr::copy_nonoverlapping(
            new_tags.as_ptr() as *const u8,
            data[tags_data_offset..].as_mut_ptr(),
            96,
        );
    }

    let data = unsafe { view.borrow_unchecked() };
    let slice: &[Address] = unsafe {
        core::slice::from_raw_parts(data[tags_data_offset..].as_ptr() as *const Address, 3)
    };
    assert_eq!(slice[2].as_array(), &[0xCC; 32]);
}

#[test]
fn dynamic_interleaved_shared_mut_shared() {
    let name = b"AB";
    let tags = [[0xCC; 32]];
    let mut buf = make_dyn_buffer_exact(name, &tags);
    let mut view = unsafe { buf.view() };

    let name_data_offset = DYN_HEADER_SIZE + 4;

    for round in 0..3u8 {
        {
            let data = unsafe { view.borrow_unchecked() };
            let name_len = u32::from_le_bytes(
                data[DYN_HEADER_SIZE..DYN_HEADER_SIZE + 4]
                    .try_into()
                    .unwrap(),
            );
            assert_eq!(name_len, 2);
        }
        {
            let data = unsafe { view.borrow_unchecked_mut() };
            data[name_data_offset] = b'A' + round;
            data[name_data_offset + 1] = b'B' + round;
        }
        {
            let data = unsafe { view.borrow_unchecked() };
            assert_eq!(data[name_data_offset], b'A' + round);
            assert_eq!(data[name_data_offset + 1], b'B' + round);
        }
    }
}

#[test]
fn dynamic_offset_cached_parse_access() {
    let name = b"hello";
    let tags: Vec<[u8; 32]> = vec![[0xAA; 32], [0xBB; 32]];
    let mut buf = make_dyn_buffer_exact(name, &tags);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    let mut offset = DYN_HEADER_SIZE;
    let mut __off: [u32; 1] = [0u32; 1];

    let name_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4 + name_len;
    __off[0] = offset as u32;

    let name_offset = DYN_HEADER_SIZE;
    let name_prefix_len =
        u32::from_le_bytes(data[name_offset..name_offset + 4].try_into().unwrap()) as usize;
    let name_start = name_offset + 4;
    let name_str =
        unsafe { core::str::from_utf8_unchecked(&data[name_start..name_start + name_prefix_len]) };
    assert_eq!(name_str, "hello");

    let tags_offset = __off[0] as usize;
    let tags_count =
        u32::from_le_bytes(data[tags_offset..tags_offset + 4].try_into().unwrap()) as usize;
    let tags_start = tags_offset + 4;
    assert_eq!(tags_count, 2);
    let tags_slice: &[Address] = unsafe {
        core::slice::from_raw_parts(data[tags_start..].as_ptr() as *const Address, tags_count)
    };
    assert_eq!(tags_slice[0].as_array(), &[0xAA; 32]);
    assert_eq!(tags_slice[1].as_array(), &[0xBB; 32]);
}

#[test]
fn dynamic_offset_cached_empty_fields() {
    let mut buf = make_dyn_buffer_exact(b"", &[]);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    let mut offset = DYN_HEADER_SIZE;
    let mut __off: [u32; 1] = [0u32; 1];

    let name_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    assert_eq!(name_len, 0);
    offset += 4;
    __off[0] = offset as u32;

    let tags_offset = __off[0] as usize;
    let tags_count =
        u32::from_le_bytes(data[tags_offset..tags_offset + 4].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 0);
    let tags_start = tags_offset + 4;
    let tags_slice: &[Address] =
        unsafe { core::slice::from_raw_parts(data[tags_start..].as_ptr() as *const Address, 0) };
    assert_eq!(tags_slice.len(), 0);
}

#[test]
fn instruction_zc_cast_exact_length() {
    let name = b"solana";
    let score: u64 = 42;
    let cap = 1 + size_of::<IxDataZc>() + 4 + name.len();
    let mut ix_data: Vec<u8> = Vec::with_capacity(cap);
    ix_data.push(0x00);
    ix_data.extend_from_slice(&score.to_le_bytes());
    ix_data.extend_from_slice(&(name.len() as u32).to_le_bytes());
    ix_data.extend_from_slice(name);
    assert_eq!(ix_data.len(), ix_data.capacity());

    let after_disc = &ix_data[1..];
    let zc = unsafe { &*(after_disc.as_ptr() as *const IxDataZc) };
    assert_eq!(zc.score.get(), 42);

    let dyn_start = size_of::<IxDataZc>();
    let dyn_len =
        u32::from_le_bytes(after_disc[dyn_start..dyn_start + 4].try_into().unwrap()) as usize;
    assert_eq!(dyn_len, 6);
    let name_start = dyn_start + 4;
    let s = core::str::from_utf8(&after_disc[name_start..name_start + dyn_len]).unwrap();
    assert_eq!(s, "solana");
}

#[test]
fn instruction_vec_arg_from_raw_parts_exact_boundary() {
    let count = 10usize;
    let cap = 1 + 4 + count * size_of::<PodU64>();
    let mut ix_data = Vec::with_capacity(cap);
    ix_data.push(0x01);
    ix_data.extend_from_slice(&(count as u32).to_le_bytes());
    for i in 0..count {
        ix_data.extend_from_slice(&(i as u64).to_le_bytes());
    }
    assert_eq!(ix_data.len(), ix_data.capacity());

    let after_disc = &ix_data[1..];
    let elem_count = u32::from_le_bytes(after_disc[..4].try_into().unwrap()) as usize;
    assert_eq!(elem_count, 10);

    let elements = &after_disc[4..];
    assert_eq!(elements.len(), count * size_of::<PodU64>());

    let slice: &[PodU64] =
        unsafe { core::slice::from_raw_parts(elements.as_ptr() as *const PodU64, count) };
    assert_eq!(slice[9].get(), 9);
    assert_eq!(slice[0].get(), 0);
}

#[test]
fn tail_str_exact_boundary() {
    let tail = b"tail data at boundary!";
    let mut buf = make_tail_buffer(tail);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    let offset = DYN_HEADER_SIZE;
    let tail_len = data.len() - offset;
    assert_eq!(tail_len, tail.len());
    let s = unsafe { core::str::from_utf8_unchecked(&data[offset..offset + tail_len]) };
    assert_eq!(s, "tail data at boundary!");
}

#[test]
fn tail_str_empty() {
    let mut buf = make_tail_buffer(b"");
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    let offset = DYN_HEADER_SIZE;
    assert_eq!(data.len() - offset, 0);
    let s = unsafe { core::str::from_utf8_unchecked(&data[offset..offset]) };
    assert_eq!(s, "");
}

#[test]
fn tail_bytes_exact_boundary() {
    let tail: &[u8] = &[0xFF, 0xFE, 0xFD, 0x00, 0x01];
    let mut buf = make_tail_buffer(tail);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    let offset = DYN_HEADER_SIZE;
    assert_eq!(&data[offset..], tail);
}

#[test]
fn tail_bytes_empty() {
    let mut buf = make_tail_buffer(b"");
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };
    assert_eq!(data[DYN_HEADER_SIZE..].len(), 0);
}

#[test]
fn tail_str_multibyte_utf8() {
    let tail = "caf\u{00e9}".as_bytes();
    let mut buf = make_tail_buffer(tail);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    let offset = DYN_HEADER_SIZE;
    let tail_len = data.len() - offset;
    let s = unsafe { core::str::from_utf8_unchecked(&data[offset..offset + tail_len]) };
    assert_eq!(s, "caf\u{00e9}");
}
