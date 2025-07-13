pub trait Decode: Sized {
    fn from_slice(data: &[u8]) -> anyhow::Result<Self>;
}

// impl<T> Decode for T
// where
//     T: bincode::Decode<()>,
// {
//     fn from_slice(data: &[u8]) -> anyhow::Result<Self> {
//         let config = bincode::config::standard().with_fixed_int_encoding();
//         let (decoded, _) = bincode::decode_from_slice::<Self, _>(data, config)?;
//         Ok(decoded)
//     }
// }

macro_rules! impl_decode {
    ($ty:ty) => {
        impl Decode for $ty {
            fn from_slice(data: &[u8]) -> anyhow::Result<Self> {
                let config = bincode::config::legacy();
                let (decoded, _) = bincode::decode_from_slice::<Self, _>(data, config)?;
                Ok(decoded)
            }
        }
    };
}

impl_decode!(Packet);
impl_decode!(Payload);

#[repr(C)]
#[derive(Debug, bincode::Decode)]
pub struct Packet {
    pub nonce: [u8; 12],
    pub data: Vec<u8>,
}

#[derive(Debug, bincode::Decode)]
pub enum Payload {
    Unknown,
    Move(i32, i32),
}
