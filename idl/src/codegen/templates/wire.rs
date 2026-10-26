// ZeroPod's wire format: little-endian scalars, u32 lengths, u8 tags.
use wincode::{config::{Configuration}, len::FixIntLen, SchemaRead, SchemaWrite};
type Config = Configuration<true, {wincode::config::DEFAULT_PREALLOCATION_SIZE_LIMIT}, FixIntLen<u32>, wincode::int_encoding::LittleEndian, wincode::int_encoding::FixInt, u8>;
const CONFIG: Config = Configuration::default().with_length_encoding::<FixIntLen<u32>>().with_tag_encoding::<u8>();
pub fn serialize_into<T>(writer: impl wincode::io::Writer, value: &T) -> wincode::WriteResult<()>
where T: SchemaWrite<Config, Src = T> + ?Sized {
    wincode::config::serialize_into(writer, value, CONFIG)
}
pub fn serialized_size<T>(value: &T) -> wincode::WriteResult<u64>
where T: SchemaWrite<Config, Src = T> + ?Sized {
    wincode::config::serialized_size(value, CONFIG)
}
pub fn deserialize<'a, T>(bytes: &'a [u8]) -> wincode::ReadResult<T>
where T: SchemaRead<'a, Config, Dst = T> {
    wincode::config::deserialize(bytes, CONFIG)
}
