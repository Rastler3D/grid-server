use std::fmt::{Debug, Formatter};
use std::mem::{MaybeUninit, replace};
use std::ops::Deref;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::Error;

pub struct Buffer<T, const N: usize>{
    filled: usize,
    data: [MaybeUninit<T>; N]
}

impl<T, const N: usize> Buffer<T,N>{
    pub fn new() -> Self{
        Self{
            filled: 0,
            data: MaybeUninit::uninit_array()
        }
    }

    #[inline(always)]
    pub fn filled(&self) -> usize{
        self.filled
    }
    pub fn push(&mut self, item: T) -> Option<Vec<T>>{
        if self.filled == N{
            let buf = replace(&mut self.data, MaybeUninit::uninit_array());
            let filled_buf = Vec::from(unsafe { MaybeUninit::array_assume_init(buf) });
            self.data[0].write(item);
            self.filled = 1;

            Some(filled_buf)
        } else {
            self.data[self.filled].write(item);
            self.filled +=1;

            None
        }
    }

    pub fn is_filled(&self) -> bool{
        self.filled == N
    }

    pub fn filled_slice(&self) -> &[T]{
        { unsafe { MaybeUninit::slice_assume_init_ref(&self.data[0..self.filled]) } }
    }

    pub fn clear(&mut self){
        self.filled = 0;
    }
}

impl<T: PartialEq, const N: usize> PartialEq<[T]> for Buffer<T,N>{
    fn eq(&self, other: &[T]) -> bool {
        self.filled_slice() == other
    }
}

impl<T: PartialEq, const N: usize> PartialEq<Buffer<T,N>> for &[T]{
    fn eq(&self, other: &Buffer<T,N>) -> bool {
         self == &other.filled_slice()
    }
}

impl<T: Debug, const N: usize> Debug for Buffer<T,N>{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.filled_slice()).finish()
    }
}

impl<T, const N: usize> From<[T;N]> for Buffer<T,N>{
    fn from(value: [T; N]) -> Self {
        Self{
            data: MaybeUninit::new(value).transpose(),
            filled: N
        }
    }
}

impl<T, const N: usize> TryInto<[T;N]> for Buffer<T,N>{
    type Error = UnfilledBuffer;

    fn try_into(self) -> Result<[T; N], Self::Error> {
        if self.filled == N{
            Ok(unsafe { MaybeUninit::array_assume_init(self.data) })
        } else {
            Err(UnfilledBuffer(N - self.filled))
        }
    }
}

pub struct UnfilledBuffer(usize);

impl<T, const N: usize> IntoIterator for Buffer<T,N>{
    type Item = T;
    type IntoIter = BufferIter<T,N>;

    fn into_iter(self) -> Self::IntoIter {
        BufferIter{
            cursor: 0,
            filled: self.filled,
            data: self.data

        }
    }
}

pub struct BufferIter<T, const N: usize>{
    filled: usize,
    cursor: usize,
    data: [MaybeUninit<T>;N]
}

impl<T, const N: usize> Iterator for BufferIter<T,N>{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor < self.filled{
            Some(unsafe { self.data.get_unchecked_mut(self.cursor).assume_init_read() })
        }  else { None }
    }
}

impl<T, const N: usize> Deref for Buffer<T,N>{
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.filled_slice()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer() {
        let mut buf = Buffer::<usize,3>::new();
        assert_eq!(buf.push(1).as_deref(), None);
        assert_eq!(buf.push(2).as_deref(), None);
        assert_eq!(buf.push(3).as_deref(), None);
        assert_eq!(buf.push(4).as_deref(), Some([1,2,3].as_slice()));
        assert_eq!(buf.filled_slice(), &[4]);
        assert_eq!(buf.push(5).as_deref(), None);
        assert_eq!(buf.filled_slice(), &[4,5]);
    }
}
