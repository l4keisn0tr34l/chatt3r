// Small, platform-neutral protocol pieces shared by future desktop transports.
// The interactive binary has opt-in bounded public-file receive.
#[path = "baseline/file_packet.rs"]
pub mod file_packet;
#[path = "baseline/file_store.rs"]
pub mod file_store;
#[path = "baseline/file_wire.rs"]
pub mod file_wire;
#[path = "baseline/text_payload.rs"]
pub mod text_payload;
