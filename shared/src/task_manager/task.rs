use::serde_derive::{Deserialize,Serialize};

#[derive(Debug,Clone,Eq,PartialEq, Serialize,Deserialize)]
pub struct TaskData{
    pub taxi: usize,
    pub combinations: [u128;2]
}

#[derive(Serialize,Deserialize,Debug)]
pub struct TaskResult{
    pub taxi: usize,
    pub result: [(u128,Distance);2]
}


pub fn split(a: u128) -> [u64; 2] {
    [a as u64,(a >> 64) as u64]
}

pub type Distance = u64;