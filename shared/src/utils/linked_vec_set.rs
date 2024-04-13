use crate::utils::vec_map::VecMap;
use super::linked_vec_map::LinkedVecMap;

#[derive(Debug)]
pub struct LinkedVecSet{
    inner: LinkedVecMap<()>
}

impl LinkedVecSet{
    pub fn new() -> Self {
        Self{
            inner: LinkedVecMap::new()
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self{
            inner: LinkedVecMap::with_capacity(capacity)
        }
    }

    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional);
    }


    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit();
    }

    pub fn insert(&mut self, k: usize) -> bool {
        self.inner.insert(k,()).is_some()
    }

    pub fn contains_key(&self, k: usize) -> bool {
        self.inner.contains_key(k)
    }

    pub fn remove(&mut self, k: usize) -> bool{
        self.inner.remove(k).is_some()
    }
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    pub fn pop_front(&mut self) -> Option<usize> {
        self.inner.pop_front().map(|x| x.0)
    }

    pub fn front(&mut self) -> Option<usize> {
        self.inner.front().map(|x| x.0)
    }
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn clear(&mut self) {
        self.inner.clear()
    }
}

impl Default for LinkedVecSet{
    fn default() -> Self {
        Self{inner: LinkedVecMap::default()}
    }
}

impl IntoIterator for LinkedVecSet{
    type Item = usize;
    type IntoIter = impl Iterator<Item = usize>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter().map(|x| x.0)
    }
}

impl IntoIterator for &LinkedVecSet{
    type Item = usize;
    type IntoIter = impl Iterator<Item = usize>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.keys().cloned()
    }
}
