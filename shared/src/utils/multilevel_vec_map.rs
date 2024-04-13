use std::hint::unreachable_unchecked;
use std::mem::replace;
#[derive(Clone, Debug)]
enum Entry<T>{
    Empty,
    Value(T),
    Level(Vec<Entry<T>>)
}

#[derive(Clone, Debug)]
pub struct MultilevelVecMap<T>{
    n: usize,
    levels_idx: usize,
    vec: Vec<Entry<T>>
}

impl<T> MultilevelVecMap<T>{
    pub fn new(levels: usize) -> Self{
        Self{
            n: 0,
            levels_idx: levels - 1,
            vec: vec![],
        }
    }

    pub fn with_capacity(capacity: usize, levels: usize) -> Self {

        Self {
            n: 0,
            levels_idx: levels - 1,
            vec: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.vec.capacity()
    }
    pub fn reserve_len(&mut self, len: usize) {
        let cur_len = self.vec.len();
        if len >= cur_len {
            self.vec.reserve(len - cur_len);
        }
    }
    pub fn reserve_len_exact(&mut self, len: usize) {
        let cur_len = self.vec.len();
        if len >= cur_len {
            self.vec.reserve_exact(len - cur_len);
        }
    }
    pub fn shrink_to_fit(&mut self) {
        // strip off trailing `None`s
        if let Some(idx) = self.vec.iter().rposition(|x| !matches!(x, Entry::Empty)) {
            self.vec.truncate(idx + 1);
        } else {
            self.vec.clear();
        }

        self.vec.shrink_to_fit()
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn clear(&mut self) {
        self.n = 0;
        self.vec.clear()
    }

    #[inline]
    pub fn get(&self, key: &[u64]) -> Option<&T> {
        if key.len() > self.levels_idx + 1{
            return None;
        }
        let mut vec = &self.vec;
        let levels = self.levels_idx;
        for level in 0..levels{
            let key = *key.get(level).unwrap_or(&0) as usize;
            match vec.get(key) {
                Some(Entry::Level(level)) => {
                    vec = level;
                },
                _ => {
                    return None;
                }
            }
        }
        let key = *key.get(self.levels_idx).unwrap_or(&0) as usize;
        match vec.get(key) {
            None | Some(Entry::Empty) => {
                return None;
            }
            Some(Entry::Value(value)) => {
                return Some(value);
            }
            Some(Entry::Level(_)) => {
                unsafe {
                    unreachable_unchecked();
                }
            }
        }
    }

    #[inline]
    pub unsafe fn get_unchecked(&self, key: &[u64]) -> &T {
        let mut vec = &self.vec;
        let levels = self.levels_idx;
        for level in 0..levels{
            let key = *key.get(level).unwrap_or(&0) as usize;
            match vec.get_unchecked(key) {
                Entry::Level(level) => {
                    vec = level;
                },
                _ => {
                    unreachable_unchecked()
                }
            }
        }
        let key = *key.get(self.levels_idx).unwrap_or(&0) as usize;
        let Entry::Value(value) = vec.get_unchecked(key) else {
            unreachable_unchecked()
        };

        value
    }

    #[inline]
    pub fn contains_key(&self, key: &[u64]) -> bool {
        self.get(key).is_some()
    }

    pub fn get_mut(&mut self, key: &[u64]) -> Option<&mut T> {
        if key.len() > self.levels_idx + 1{
            return None;
        }
        let mut vec = &mut self.vec;
        let levels = self.levels_idx;
        for level in 0..levels{
            let key = *key.get(level).unwrap_or(&0) as usize;
            match vec.get_mut(key) {
                Some(Entry::Level(level)) => {
                    vec = level;
                },
                _ => {
                    return None;
                }
            }
        }
        let key = *key.get(self.levels_idx).unwrap_or(&0) as usize;
        match vec.get_mut(key) {
            None | Some(Entry::Empty) => {
                return None;
            }
            Some(Entry::Value(value)) => {
                return Some(value);
            }
            Some(Entry::Level(_)) => {
                unsafe {
                    unreachable_unchecked();
                }
            }
        }
    }

    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self, key: &[u64]) -> &mut T {
        let mut vec = &mut self.vec;
        let levels = self.levels_idx;
        for level in 0..levels{
            let key = *key.get(level).unwrap_or(&0) as usize;
            match vec.get_unchecked_mut(key) {
                Entry::Level(level) => {
                    vec = level;
                },
                _ => {
                    unreachable_unchecked()
                }
            }
        }
        let key = *key.get(self.levels_idx).unwrap_or(&0) as usize;
        let Entry::Value(value) = vec.get_unchecked_mut(key) else {
            unreachable_unchecked()
        };
        
        value
    }

    pub fn insert(&mut self, key: &[u64], value: T) -> Option<T> {
        if key.len() > self.levels_idx + 1{
            return None;
        }
        let levels = self.levels_idx;
        let mut vec = &mut self.vec;
        
        for level in 0..levels {
            let len = vec.len();
            let key = *key.get(level).unwrap_or(&0) as usize;
        
            if len <= key {
                vec.extend((0..key - len + 1).map(|_| Entry::Level(Vec::new())));
            }
            let Entry::Level(level) = (unsafe { vec.get_unchecked_mut(key) }) else {
                unsafe { unreachable_unchecked() }               
            };
            vec = level;
        }

        let len = vec.len();
        let key = *key.get(self.levels_idx).unwrap_or(&0) as usize;

        if len <= key {
            vec.extend((0..key - len + 1).map(|_| Entry::Empty));
            
        }
        
        match unsafe { vec.get_unchecked_mut(key)} {
            entry@Entry::Empty => {
                *entry = Entry::Value(value);
            },
            Entry::Value(x) => {
                let value = replace(x,value);
                
                
                return Some(value)
            }
            Entry::Level(_) => {
                unsafe{ unreachable_unchecked() }
            }
        };
        
        self.n+=1;
        return None;
    }

    pub fn remove(&mut self, key: &[u64]) -> Option<T> {
        if key.len() > self.levels_idx + 1{
            return None;
        }
        let levels = self.levels_idx;
        let mut vec = &mut self.vec;
        for level in 0..levels{
            let key = *key.get(level).unwrap_or(&0) as usize;
            match vec.get_mut(key) {
                Some(Entry::Level(level)) => {
                    vec = level;
                },
                _ => {
                    return None;
                }
            }
        }
        let key = *key.get(self.levels_idx).unwrap_or(&0) as usize;
        
        let old = match vec.get_mut(key) {
            None | Some(Entry::Empty) => {
                return None;
            },
            Some(x) => {
                replace(x,Entry::Empty)
            }
        };
        self.n -=1;
        let Entry::Value(old) = old else { 
            unsafe { unreachable_unchecked() }
        };
        
        Some(old)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multilevel_vec_map_insert() {
        let mut map = MultilevelVecMap::new(3);
        map.insert(&[1,2,3], 10);
        map.insert(&[2,3,4],15);

        assert_eq!(map.get(&[1,2,3]), Some(&10));
       unsafe { assert_eq!(map.get_unchecked(&[2, 3, 4]), &15); }

    }

    #[test]
    fn multilevel_vec_map_remove() {
        let mut map = MultilevelVecMap::new(3);
        map.insert(&[1,2,3], 10);


        assert_eq!(map.remove(&[1,2,3]), Some(10));
        assert_eq!(map.get(&[1, 2, 3]), None)

    }

    #[test]
    fn multilevel_vec_map_get() {
        let mut map = MultilevelVecMap::new(3);
        map.insert(&[3,0,0], 10);
        map.insert(&[2,1], 11);
        map.insert(&[1], 12);


        assert_eq!(map.get(&[3]), Some(&10));
        assert_eq!(map.get(&[2,1,0]), Some(&11));
        assert_eq!(map.get(&[1,0,0]), Some(&12));
        assert_eq!(map.get(&[1,0,1]), None);

    }
}