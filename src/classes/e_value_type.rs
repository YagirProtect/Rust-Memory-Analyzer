#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EValueType {
    #[default]
    I32,
    I64,
    F32,
    F64,
    Utf8String,
    Bytes,
}