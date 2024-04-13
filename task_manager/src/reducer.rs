use std::fmt::{Debug, Formatter};
use std::iter;
use shared::task_manager::task::Distance;
use shared::utils::multilevel_vec_map::MultilevelVecMap;
use bitset_core::BitSet;

pub struct TaskReducer {
    min: Distance,
    buf: Vec<Vec<u64>>,
    combinations: CombinationGenerator,
    completed_parts: Vec<MultilevelVecMap<Distance>>
}
impl TaskReducer {

    pub fn new(taxies: usize, passengers: usize, completed_parts: Vec<MultilevelVecMap<Distance>>) -> Self{
        let size = passengers / 64 + 1;
        let combinations = CombinationGenerator::new(taxies,passengers);
        Self{
            buf: vec![vec![0;size];taxies],
            min: Distance::MAX,
            combinations,
            completed_parts,
        }
    }
}

impl Iterator for TaskReducer{
    type Item = Distance;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(combination) = self.combinations.next(){
            for Pair{ taxi, passenger} in combination{
                unsafe { self.buf.get_unchecked_mut(taxi).bit_set(passenger) };
            }
            let mut distance = 0;
            for (taxi,passangers) in self.buf.iter_mut().enumerate().filter(|(_,x)| x.bit_any()){
                distance += self.completed_parts.get(taxi).and_then(|x| x.get(passangers)).unwrap();
                passangers.bit_init(false);
            }
            return Some(distance);
        }
        None
    }
}


pub struct CombinationGenerator{
    n: u128,
    taxies: usize,
    passengers: usize,
}

impl CombinationGenerator{

    pub fn new(taxies: usize, passengers: usize) -> Self{
        Self{
            n: (taxies as u128).pow(passengers as u32),
            taxies,
            passengers,
        }
    }
    fn extract_combination(&mut self) -> impl Iterator<Item = Pair>{
        let taxies = self.taxies as u128;
        let mut passenger = self.passengers;
        let mut num = self.n;


        iter::from_fn(move ||{
            if passenger == 0{
                None
            } else {
                let taxi = (num % taxies) as usize;
                num = num / taxies;
                let pair = Pair { passenger: passenger - 1, taxi };
                passenger -= 1;

                Some(pair)
            }
        })
    }
}

impl Iterator for CombinationGenerator{
    type Item = impl Iterator<Item = Pair>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.n == 0{
            None
        } else {
            self.n -=1;
            Some(self.extract_combination())
        }
    }
}


#[derive(Copy, Clone)]
pub struct Pair{
    pub taxi: usize,
    pub passenger: usize
}

impl Debug for Pair {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .key(&"taxi").value(&self.taxi)
            .key(&"passenger").value(&self.passenger)
            .finish()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combinations() {
        let mut a = CombinationGenerator::new(7, 9);
        let mut b = 0;
        let res: usize = a.map(|x| x.count()).sum();
        println!("{:?}", res);
    }
}