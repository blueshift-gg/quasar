pub mod make_event;
pub mod refund_event;
pub mod take_event;

pub use make_event::*;
pub use refund_event::*;
pub use take_event::*;

pub enum ProgramEvent {
    MakeEvent(MakeEvent),
    RefundEvent(RefundEvent),
    TakeEvent(TakeEvent),
}

pub fn decode_event(data: &[u8]) -> Option<ProgramEvent> {
    if data.starts_with(MAKE_EVENT_DISCRIMINATOR) {
        let value = wincode::deserialize::<MakeEvent>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramEvent::MakeEvent(value));
    }
    if data.starts_with(REFUND_EVENT_DISCRIMINATOR) {
        let value = wincode::deserialize::<RefundEvent>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramEvent::RefundEvent(value));
    }
    if data.starts_with(TAKE_EVENT_DISCRIMINATOR) {
        let value = wincode::deserialize::<TakeEvent>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramEvent::TakeEvent(value));
    }
    None
}
