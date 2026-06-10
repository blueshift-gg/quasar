pub mod make_event;
pub mod refund_event;
pub mod take_event;

pub use {make_event::*, refund_event::*, take_event::*};

pub enum ProgramEvent {
    MakeEvent(MakeEvent),
    TakeEvent(TakeEvent),
    RefundEvent(RefundEvent),
}

pub fn decode_event(data: &[u8]) -> Option<ProgramEvent> {
    if data.starts_with(MAKE_EVENT_DISCRIMINATOR) {
        return wincode::deserialize::<MakeEvent>(data)
            .ok()
            .map(ProgramEvent::MakeEvent);
    }
    if data.starts_with(TAKE_EVENT_DISCRIMINATOR) {
        return wincode::deserialize::<TakeEvent>(data)
            .ok()
            .map(ProgramEvent::TakeEvent);
    }
    if data.starts_with(REFUND_EVENT_DISCRIMINATOR) {
        return wincode::deserialize::<RefundEvent>(data)
            .ok()
            .map(ProgramEvent::RefundEvent);
    }
    None
}
