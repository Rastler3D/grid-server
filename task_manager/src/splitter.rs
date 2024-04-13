use shared::task_manager::task::TaskData;

#[derive(Debug)]
pub struct TaskSplitter{
    n: usize,
    taxies: usize,
    cur_taxi: usize,
    max: u128,
    mid: u128,
    cur: u128
}

impl TaskSplitter{

    pub fn total_tasks(&self) -> u128{
        self.mid * self.taxies as u128
    }
    pub fn new(taxies: usize, passengers: usize) -> Self {
        let (max,mid) = {
            let comb = 2u128.pow(passengers as u32);
            (comb-1, comb/2)
        };

        TaskSplitter{
            n: 0,
            cur_taxi: 0,
            cur:mid,
            taxies,
            mid,
            max,
        }
    }

}

impl Iterator for TaskSplitter{
    type Item = (usize,TaskData);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_taxi < self.taxies{
            let data = Some((self.n,TaskData{
                taxi: self.cur_taxi,
                combinations: [self.cur, self.max^self.cur]
            }));
            self.n += 1;
            if self.cur == self.max{
                self.cur_taxi+=1;
                self.cur = self.mid;
            } else {
                self.cur+=1;
            }

            data
        } else {
            None
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn splits() {
        let mut a = TaskSplitter::new(2, 4);
        let mut b = 0;
        for i in a{
            println!("{:?} ", i);
            b+=1;
        }
        println!("{}", b);
    }
}