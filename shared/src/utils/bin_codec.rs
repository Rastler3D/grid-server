use std::fmt::{Debug, Formatter, Pointer};
use std::io;
use std::io::{ErrorKind};
use std::marker::PhantomData;
use bincode::config::Configuration;

use serde::{Deserialize, Serialize};
use tokio_util::bytes::{Buf, Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};


pub struct BinCodec<T>
where
    for <'de> T: Deserialize<'de> + Serialize
{
    _phantom: PhantomData<T>,
    length_codec: LengthDelimitedCodec,
    config: Configuration
}

impl<T> BinCodec<T>
where
    for <'de> T: Deserialize<'de> + Serialize,
{
    pub fn new() -> Self{

        Self{
            _phantom: PhantomData,
            length_codec: LengthDelimitedCodec::builder()
                .length_field_type::<u64>()
                .max_frame_length(usize::MAX)
                .new_codec(),
            config: bincode::config::standard()
        }
    }
}

impl<T> Decoder for BinCodec<T>
where
    for <'de> T: Deserialize<'de> + Serialize
{
    type Item = T;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match self.length_codec.decode(src)? {
            None => Ok(None),
            Some(bytes) => {
                match bincode::serde::decode_from_std_read(&mut bytes.reader(),self.config){
                    Ok(item) => Ok(Some(item)),
                    Err(err) => Err(io::Error::new(ErrorKind::InvalidData,err))
                }
            }
        }
    }
}

impl<T> Encoder<T> for BinCodec<T>
where
    for <'de> T: Deserialize<'de> + Serialize
{
    type Error = io::Error;

    fn encode(&mut self, item: T, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match bincode::serde::encode_to_vec(item, self.config){
            Ok(bytes) => self.length_codec.encode(Bytes::from(bytes),dst),
            Err(err) => Err(io::Error::new(ErrorKind::InvalidData,err))
        }
    }
}

impl<T> Debug for BinCodec<T>
where
    for <'de> T: Deserialize<'de> + Serialize
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BinCodec").finish()
    }
}


