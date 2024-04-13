use std::borrow::Borrow;
use std::hash::{Hash, RandomState};
use std::marker::PhantomData;
use std::mem::{replace, swap};
use std::ops::{Add, Deref, DerefMut};
use std::ptr::swap_nonoverlapping;
use crate::utils::vec_map::VecMap;

type Id = usize;
type Priority = usize;
type HeapPos = usize;
pub struct Task;



#[derive(Debug,Clone)]
pub struct PriorityQueue<Value: Ord,Order: Ordering = Max>
{
    _phantom: PhantomData<Order>,
    pub map: VecMap<(HeapPos,Value)>,
    pub heap: Vec<Id>,
}

impl<Value: Ord, Order: Ordering> PriorityQueue<Value, Order>{
    #[inline(always)]
    pub fn new() -> PriorityQueue<Value,Order>{
        PriorityQueue{
            _phantom: PhantomData,
            map: VecMap::new(),
            heap: Vec::new(),
            //size: 0,
        }
    }

    #[inline(always)]
    pub fn new_max() -> PriorityQueue<Value, Max>{
        PriorityQueue{
            _phantom: PhantomData,
            map: VecMap::new(),
            heap: Vec::new(),
            //size: 0,
        }
    }
    #[inline(always)]
    pub fn new_min() -> PriorityQueue<Value, Min>{
        PriorityQueue{
            _phantom: PhantomData,
            map: VecMap::new(),
            heap: Vec::new(),
           // size: 0,
        }
    }
    #[inline(always)]
    pub fn with_capacity(capacity: usize) -> Self {
        Self{
            _phantom: PhantomData,
            map: VecMap::with_capacity(capacity),
            heap: Vec::with_capacity(capacity),
        }
    }

    #[inline(always)]
    pub fn clear(&mut self){
        self.heap.clear();
        self.map.clear();
    }

    #[inline(always)]
    pub fn reserve_exact(&mut self, additional: usize){
        self.heap.reserve_exact(additional);
        self.map.reserve_len_exact(additional);
    }

    pub fn get(&self, id: Id) -> Option<&Value>{
        self.map.get(id).map(|x| &x.1)
    }

    pub fn get_mut(&mut self, id: Id) -> Option<RefMut<Value, Order>>{
        if self.map.contains_key(id) {
            Some(RefMut {
                key: id,
                modified: false,
                queue: self
            })
        } else {
            None
        }
    }
    #[inline(always)]
    pub fn peek(&self) -> Option<(Id, &Value)> {
        let id = *self.heap.get(0)?;

        Some((id, unsafe{ &self.map.get_unchecked(id).1 }))
    }

    pub fn peek_mut(&mut self) -> Option<PeekMut<Value,Order>> {
        let id = *self.heap.get(0)?;
        Some(PeekMut{
            key: id,
            modified: false,
            queue: self
        })
    }

    pub fn map<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Value)
    {
        for (_,value) in &mut self.map{
            f(value);
        }
        for i in 0..self.heap.len()
        {
            self.up_heapify(i);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Value>{
        self.heap.iter().map(|&x| unsafe { self.map.get_unchecked(x) }).map(|x| &x.1)
    }
    #[inline(always)]
    pub fn pop(&mut self) -> Option<(Id, Value)> {
        if self.heap.len() == 0 {
            None
        } else {
            let r = unsafe { self.swap_remove(0) };
            self.heapify(0);
            Some(r)
        }
    }
    #[inline(always)]
    pub fn push(&mut self, id: Id, priority: Value) -> Option<Value> {

        if let Some((pos, old_priority)) =  self.map.get_mut(id){
            let pos = *pos;
            let oldp = Some(replace(old_priority, priority));
            self.up_heapify(pos);
            return oldp;
        }

        let i = self.heap.len();
        self.heap.push((id));
        self.map.insert(id,(i,priority));

        self.bubble_up(i);

        None
    }
    #[inline(always)]
    pub fn remove(&mut self, id: Id) -> Option<(Id, Value)> {
        match self.map.get(id){
            Some(&(pos,_)) => {
                let (id,priority) = unsafe{ self.swap_remove(pos) };
                if pos < self.heap.len() {
                    self.up_heapify(pos);
                }
                Some((id, priority))
            },
            None => None
        }
    }

    pub fn change_priority(&mut self, id: Id, mut new_priority: Value) -> Option<Value>
    {
        self.map.get_mut(id).map(|(position,priority)| {
            swap(priority, &mut new_priority);

            (new_priority, *position)
        }).map(|(priority, pos)| {
            self.up_heapify(pos);

            priority
        })
    }

    pub fn change_priority_by<F>(&mut self, id: Id, priority_setter: F) -> bool
    where
        F: FnOnce(&mut Value),
    {
        self.map.get_mut(id).map(|(position,priority)| {
            priority_setter(priority);

            *position
        }).map(|pos| {
            self.up_heapify(pos);
        }).is_some()
    }

    #[inline(always)]
    unsafe fn swap_remove(&mut self, position: HeapPos) -> (Id, Value) {
        let last = self.heap.len() - 1;
        unsafe {
            let new = *self.heap.get_unchecked(last);
            self.map.get_unchecked_mut(new).0 = position;
        }
        let head = self.heap.swap_remove(position);
        let (_, value) = unsafe{ self.map.remove_unchecked(head) };


        return (head,value);
    }

    #[inline(always)]
    fn up_heapify(&mut self, i: HeapPos) {
        let pos = self.bubble_up(i);
        self.heapify(pos)
    }

    #[inline(always)]
    fn heapify(&mut self, mut i: HeapPos) {
        if self.heap.len() <= 1 {
            return;
        }
        let mut largest = i;
        loop {
            let mut largestp = unsafe { self.get_priority_from_position(i) };
            let  l = left(i);
            if l < self.heap.len() {
                let childp = unsafe { self.get_priority_from_position(l) };
                if Order::cmp_priority(largestp, childp) {
                    largest = l;
                    largestp = childp;
                }

                let r = right(i);
                if r < self.heap.len()
                    && Order::cmp_priority(largestp, unsafe { self.get_priority_from_position(r) })
                {
                    largest = r;
                }
            }
            if largest != i{
                self.swap(i, largest);
                i = largest;

                continue;
            }

            break;
        }
    }
    #[inline(always)]
    fn bubble_up(&mut self, mut position: HeapPos) -> HeapPos {
        while position > 0 {
            let mut parent_position = parent(position);
            let [current_heap,parent_heap] = unsafe{ self.heap.get_many_unchecked_mut([position,parent_position])};
            let [current, parent] = unsafe { self.map.get_many_unchecked_mut([*current_heap,*parent_heap]) };
            if Order::cmp_priority(&parent.1,&current.1){
                unsafe {
                    swap_nonoverlapping(current_heap,parent_heap,1);

                    parent.0 = position;
                }
                position = parent_position;
            } else { break }


        }
        unsafe {
            let id = *self.heap.get_unchecked_mut(position);
            self.map.get_unchecked_mut(id).0 = position;
        }

        position
    }

    #[inline(always)]
    pub unsafe fn get_priority_from_position(&self, position: HeapPos) -> &Value {
        let id = *self.heap.get_unchecked(position);
        &self.map.get_unchecked(id).1
    }

    #[inline(always)]
    pub fn swap(&mut self, a: HeapPos, b: HeapPos) {
        unsafe {
            let [a,b] = unsafe { self.heap.get_many_unchecked_mut([a,b]) };
            let [(a1,_),(b1,_)] = unsafe { self.map.get_many_unchecked_mut([*a,*b]) };
            swap_nonoverlapping(a,b,1);
            swap_nonoverlapping(a1,b1,1);
        }
    }
}

impl<Order:Ordering> PriorityQueue<usize, Order>{
    pub fn increment_priority(&mut self, id: Id) -> Option<usize>{
        let mut prev_priority = None;
        self.change_priority_by(id, |mut x| {
            prev_priority = Some(*x);
            *x+=1
        });

        prev_priority
    }
    pub fn decrement_priority(&mut self, id: Id) -> Option<usize>{
        let mut prev_priority = None;
        self.change_priority_by(id, |mut x| {
            prev_priority = Some(*x);
            *x-=1
        });

        prev_priority
    }
}

pub struct PeekMut<'a,Value:Ord, Order: Ordering>{
    queue: &'a mut PriorityQueue<Value,Order>,
    key: usize,
    modified: bool
}

impl<'a,Value:Ord, Order: Ordering> Deref for PeekMut<'a,Value,Order>{
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        unsafe { &self.queue.map.get_unchecked(self.key).1 }
    }
}

impl<'a,Value:Ord, Order: Ordering> DerefMut for PeekMut<'a,Value,Order>{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.modified = true;
        unsafe { &mut self.queue.map.get_unchecked_mut(self.key).1 }
    }
}

impl<'a,Value:Ord, Order: Ordering> Drop for PeekMut<'a,Value,Order> {
    fn drop(&mut self) {
        if self.modified{
            self.queue.heapify(0);
        }
    }
}

pub struct RefMut<'a,Value:Ord, Order: Ordering>{
    queue: &'a mut PriorityQueue<Value,Order>,
    key: usize,
    modified: bool
}

impl<'a,Value:Ord, Order: Ordering> Deref for RefMut<'a, Value, Order> {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        unsafe { &self.queue.map.get_unchecked(self.key).1 }
    }
}

impl<'a,Value:Ord, Order: Ordering> DerefMut for RefMut<'a, Value, Order> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.modified = true;
        unsafe { &mut self.queue.map.get_unchecked_mut(self.key).1 }
    }
}

impl<'a,Value:Ord, Order: Ordering> Drop for RefMut<'a, Value, Order> {
    fn drop(&mut  self) {
        if self.modified{
            if let Some(&(pos,_)) = self.queue.map.get(self.key){
                self.queue.heapify(pos);
            }
        }
    }
}


#[inline(always)]
const fn left(i: HeapPos) -> HeapPos {
    ((i * 2) + 1)
}
/// Compute the index of the right child of an item from its index
#[inline(always)]
const fn right(i: HeapPos) -> HeapPos {
    ((i * 2) + 2)
}
/// Compute the index of the parent element in the heap from its index
#[inline(always)]
const fn parent(i: HeapPos) -> HeapPos {
    ((i - 1) / 2)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use super::*;

    #[test]
    fn test_queue() {
        let mut queue = PriorityQueue::<_,Max>::new();

        queue.push(1, 10);
        queue.push(3,1);
        queue.push(6,1);
        queue.push(1, 8);
        queue.push(7,3);
        queue.push(8,5);
        queue.push(2, 10);
        queue.push(3,1);
        queue.push(4,7);
        queue.push(5, 6);
        queue.push(9,2);
        queue.push(10,4);
        queue.push(10,4);
        queue.push(11,4);


        println!("{:#?}", queue);
        while let Some(x) = queue.pop() {
            println!("{:?}", x);
        }
    }
}
trait Ordering{
    #[inline(always)]
    fn cmp_priority<Value: Ord>(lower: Value, larger: Value) -> bool;
}
#[derive(Debug,Clone)]
pub struct Max;
impl Ordering for Max{
    #[inline(always)]
    fn cmp_priority<Value: Ord>(lower: Value, larger: Value) -> bool{
        lower < larger
    }
}

#[derive(Debug,Clone)]
pub struct Min;
impl Ordering for Min{
    #[inline(always)]
    fn cmp_priority<Value: Ord>(lower: Value, larger: Value) -> bool{
        lower > larger
    }
}


