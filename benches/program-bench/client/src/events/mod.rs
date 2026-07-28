pub mod large_event;
pub mod multi_event;
pub mod second_simple_event;
pub mod simple_event;

pub use large_event::*;
pub use multi_event::*;
pub use second_simple_event::*;
pub use simple_event::*;

pub const EMPTY_EVENT_DISCRIMINATOR: &[u8] = &[5];

pub enum ProgramEvent {
    EmptyEvent,
    LargeEvent(LargeEvent),
    MultiEvent(MultiEvent),
    SecondSimpleEvent(SecondSimpleEvent),
    SimpleEvent(SimpleEvent),
}

pub fn decode_event(data: &[u8]) -> Option<ProgramEvent> {
    if data.starts_with(EMPTY_EVENT_DISCRIMINATOR) {
        return (data.len() == EMPTY_EVENT_DISCRIMINATOR.len())
            .then_some(ProgramEvent::EmptyEvent);
    }
    if data.starts_with(LARGE_EVENT_DISCRIMINATOR) {
        let value = wincode::deserialize::<LargeEvent>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramEvent::LargeEvent(value));
    }
    if data.starts_with(MULTI_EVENT_DISCRIMINATOR) {
        let value = wincode::deserialize::<MultiEvent>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramEvent::MultiEvent(value));
    }
    if data.starts_with(SECOND_SIMPLE_EVENT_DISCRIMINATOR) {
        let value = wincode::deserialize::<SecondSimpleEvent>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramEvent::SecondSimpleEvent(value));
    }
    if data.starts_with(SIMPLE_EVENT_DISCRIMINATOR) {
        let value = wincode::deserialize::<SimpleEvent>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramEvent::SimpleEvent(value));
    }
    None
}
