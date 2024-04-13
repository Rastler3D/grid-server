#![feature(test)]
#![feature(let_chains)]
#![feature(assert_matches)]
extern crate test;

pub mod vec_map{
    use std::collections::BinaryHeap;
    use test::{Bencher, black_box};
    use grid_server::vec_map::VecMap;

    #[bench]
    fn push_and_pop_vec(b: &mut Bencher) {
        let mut pq = Vec::from([0]);
        let mut i = 0;
        b.iter(|| unsafe {
            i+=1;
            black_box(black_box(&mut pq).push(black_box(i)));
            black_box(pq.get(i));

        });
    }
    #[bench]
    fn push_and_pop_vec_map(b: &mut Bencher) {
        let mut pq = VecMap::new();
        let mut i = 0;
        b.iter(|| unsafe {
            i+=1;
            pq.insert(i,10);
            black_box(pq.get(i));

        });
    }
}

pub mod priority_queue {
    use std::collections::BinaryHeap;
    use std::hint::unreachable_unchecked;
    use std::panic::catch_unwind;
    use test::{Bencher, black_box};
    use grid_server::vec_map::VecMap;
    use grid_server::priority_queue::{Max, PriorityQueue};
    #[bench]
    fn push_and_pop_bh(b: &mut Bencher) {
        let mut pq = BinaryHeap::new();
        b.iter(|| {
            pq.push(black_box(0));
            assert_eq![pq.pop().unwrap(), 0];
        });
    }

    #[bench]
    fn push_and_pop_pq(b: &mut Bencher) {
        let mut pq = PriorityQueue::<usize,Max>::new();
        b.iter(|| {
            pq.push(black_box(0), black_box(0));
            assert_eq![pq.pop().unwrap().1, 0];
        });
    }

    #[bench]
    fn push_and_pop_cpq(b: &mut Bencher) {
        let mut pq = priority_queue::PriorityQueue::new();
        b.iter(|| {
            pq.push(black_box(0), black_box(0));
            assert_eq![pq.pop().unwrap().1, 0];
        });
    }

    #[bench]
    fn push_and_pop_vec(b: &mut Bencher) {
        let mut pq = Vec::new();
        b.iter(|| {
            pq.push(black_box(0));
            assert_ne![pq.pop().unwrap(), 100001];
        });
    }

    #[bench]
    fn push_and_pop_on_large_bh(b: &mut Bencher) {
        let mut pq = BinaryHeap::new();
        for i in 0..100_000 {
            pq.push(black_box(i as usize));
        }
        b.iter(|| {
            pq.push(black_box(0));
        });
    }

    #[bench]
    fn push_and_pop_on_large_bh_random(b: &mut Bencher) {
        let mut pq = BinaryHeap::new();
        for i in 0..100_000 {
            pq.push(black_box(i as usize));
        }
        b.iter(|| {
            for i in 0..100 {
                pq.push(black_box(i as usize % 10));
            }
        });
    }

    #[bench]
    fn push_and_pop_on_large_pq_random(b: &mut Bencher) {
        let mut pq = PriorityQueue::<usize,Max>::new();
        for i in 1..100_000 {
            pq.push(black_box(i as usize), black_box(i));
        }
        b.iter(|| {
            for i in 1..100 {
                pq.push(black_box(i % 10), black_box(i));
            }
        });
    }

    #[bench]
    fn push_and_pop_on_large_cpq_random(b: &mut Bencher) {
        let mut pq = priority_queue::PriorityQueue::new();
        for i in 1..100_000 {
            pq.push(black_box(i as usize), black_box(i));
        }
        b.iter(|| {
            for i in 1..100 {
                pq.push(black_box(i as usize % 10), black_box(i));
            }
        });
    }

    #[bench]
    fn push_and_pop_on_large_vec(b: &mut Bencher) {
        let mut pq = Vec::new();
        for i in 0..100_000 {
            pq.push(black_box(i as usize));
        }
        b.iter(|| {
            pq.push(black_box(0));
            assert_eq![pq.pop().unwrap(), black_box(0)];
        });
    }

    #[bench]
    fn push_and_pop_on_large_pq(b: &mut Bencher) {
        let mut pq = PriorityQueue::<usize,Max>::new();
        for i in 1..100_000 {
            pq.push(black_box(i as usize), black_box(i));
        }
        b.iter(|| {
            pq.push(black_box(0), black_box(0));
            assert_ne![pq.pop().unwrap().0, 1000001];
        });
    }

    #[bench]
    fn push_and_pop_on_large_cpq(b: &mut Bencher) {
        let mut pq = priority_queue::PriorityQueue::new();
        for i in 1..100_000 {
            pq.push(black_box(i as usize), black_box(i));
        }
        b.iter(|| {
            pq.push(black_box(0), black_box(0));
            assert_ne![pq.pop().unwrap().0, 1000001];
        });
    }

    #[bench]
    fn priority_change_on_large_queue_std(b: &mut Bencher) {
        use std::collections::BinaryHeap;
        struct Entry(usize, i32);
        impl Ord for Entry {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.0.cmp(&other.0)
            }
        }
        impl PartialOrd for Entry {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                self.0.partial_cmp(&other.0)
            }
        }
        impl Eq for Entry {}
        impl PartialEq for Entry {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        let mut pq = BinaryHeap::new();
        for i in 0..100_000 {
            pq.push(Entry(black_box(i as usize), black_box(i)));
        }
        b.iter(|| {
            let mut entry = black_box(Entry(0, 0));
            black_box(pq.retain(|x| {
                if x.0 == 50_000 {
                    entry.0 = x.0 / 2;
                    entry.1 = x.1;
                    false
                } else { true }
            }));
            black_box(pq.push(entry));
        });
    }

    #[bench]
    fn priority_change_on_large_cpq(b: &mut Bencher) {
        let mut pq = priority_queue::PriorityQueue::new();
        for i in 0..100_000 {
            pq.push(black_box(i as usize), black_box(i));
        }
        b.iter(|| {
            black_box(pq.change_priority_by(black_box(&50_000), |p| *p = *p / 2));
        });
    }

    #[bench]
    fn priority_change_on_large_pq(b: &mut Bencher) {
        let mut pq = PriorityQueue::<usize,Max>::new();
        for i in 0..100_000 {
            pq.push(black_box(i as usize), black_box(i));
        }
        b.iter(|| {
            black_box(pq.change_priority_by(black_box(50_000), |p| *p = *p / 2));
        });
    }
}

pub mod expiring_queue{
    use std::collections::{BinaryHeap, BTreeMap, BTreeSet};
    use test::black_box;
    use grid_server::priority_queue::{Min, PriorityQueue};

    #[bench]
    fn heap(b: &mut test::Bencher) {

        let mut queue = black_box(BinaryHeap::new());
        for i in 0..1000000{
            queue.push(i);
        }
        let mut i = 0;
        b.iter(||{
            i+=1;
            for i in 0..100{
                black_box(queue.pop());
                queue.push(i);

            }

            // let mut vec = black_box(Vec::new());
            // loop {
            //     let a = heap.pop();
            //     if let Some(a) = a && a < 100 {
            //         vec.push(a);
            //         continue;
            //     }
            //     break;
            // }
        });
        println!("{}", i);
    }
    #[bench]
    fn priority_queue(b: &mut test::Bencher) {

        let mut queue = black_box(PriorityQueue::<usize,Min>::with_capacity(1_000_000_000));
        for i in 0..1000000{
            queue.push(i,i);
        }
        let mut i = 0;
        b.iter(||{
            i+=1;
            for i in 0..100{
                black_box(queue.pop());
                queue.push(i,i);

            }


            // let mut vec = black_box(Vec::new());
            // loop {
            //     let a = queue.pop();
            //     if let Some(a) = a && a.0 < 100 {
            //         vec.push(a);
            //         continue;
            //     }
            //     break;
            // }
        });
        println!("{}", i);
    }

    #[bench]
    fn btree(b: &mut test::Bencher) {
        let mut btree = BTreeSet::new();
        for i in 0..1{
            btree.insert(i);
        }
        b.iter(||{
                let mut btree = btree.clone();
                let vec = black_box(btree.split_off(&100).into_iter().collect::<Vec<_>>());

        });
    }

}

pub mod multilevel_vec_map{
    use std::collections::HashMap;
    use std::hint::black_box;
    use grid_server::multilevel_vec_map::MultilevelVecMap;
    use grid_server::vec_map::VecMap;

    #[bench]
    fn multilevel(b: &mut test::Bencher) {
        let mut map = MultilevelVecMap::new(10);
        let mut i = 0;
        b.iter(||{
            map.insert(&[i,0,0,0,0,0,0,0,0,0],i);
            i+=1;
            map.remove(&[i,0,0]);
        });
    }

    #[bench]
    fn multilevel_flat(b: &mut test::Bencher) {
        let mut map = MultilevelVecMap::new(1);
        let mut i = 0;
        b.iter(||{
            map.insert(&[i],i);
            i+=1;
            map.remove(&[i]);
        });
    }

    #[bench]
    fn hashmap(b: &mut test::Bencher) {
        let mut map = HashMap::new();
        let mut i = 0;
        b.iter(||{
            map.insert([i,0,0,0,0,0,0,0,0,0],i);
            i+=1;
            map.remove(&[i,0,0,0,0,0,0,0,0,0]);
        });
    }

    #[bench]
    fn vec_map(b: &mut test::Bencher) {
        let mut map = VecMap::new();
        let mut i = 0;
        b.iter(||{
            map.insert(i,i);
            i+=1;
            map.remove(i);
        });
    }

    #[bench]
    fn multilevel_get(b: &mut test::Bencher) {
        let mut map = MultilevelVecMap::new(10);
        let mut i = 0;
        for i in 0..=10000{
            map.insert(&[i,0,0,0,0,0,0,0,0,0],i);
        }
        b.iter(||{
            let a = black_box(map.get(&[i,0,0,0,0,0,0,0,0,0])).unwrap();
            i=i%10000 + 1;

        });
    }

    #[bench]
    fn multilevel_flat_get(b: &mut test::Bencher) {
        let mut map = MultilevelVecMap::new(1);
        let mut i = 0;
        for i in 0..=10000{
            map.insert(&[i],i);
        }
        b.iter(||{
            let a =black_box(map.get(&[i])).unwrap();
            i=i%10000 + 1;
        });
    }

    #[bench]
    fn hashmap_get(b: &mut test::Bencher) {
        let mut map = HashMap::new();
        let mut i = 0;
        for i in 0..=10000{
            map.insert([i,0,0,0,0,0,0,0,0,0],i);
        }
        b.iter(||{
            let a  =black_box(map.get(&[i,0,0,0,0,0,0,0,0,0])).unwrap();
            i=i%10000 + 1;
        });
    }

    #[bench]
    fn vec_map_get(b: &mut test::Bencher) {
        let mut map = VecMap::new();
        let mut i = 0;
        for i in 0..=10000{
            map.insert(i,i);
        }
        b.iter(||{
            let a = black_box(map.get(i)).unwrap();
            i=i%10000 + 1;
        });
    }
}

mod combination_generator{
    use std::assert_matches::assert_matches;
    use std::hint::black_box;
    use bitset_core::BitSet;
    use grid_server::task_manager::{CombinationGenerator, Pair};
    use grid_server::vec_map::VecMap;

    #[bench]
    fn combinations(b: &mut test::Bencher) {
        const  size: usize = 5/128 + 1;
        let mut vec = vec![vec![0u64;size];5];
        // for i in 0..5{
        //     vec.insert(i, vec![0u64;6]);
        // }
        b.iter(|| unsafe {
            let mut a = CombinationGenerator::new(5, 6);
            let mut i = 0;

            for i in a{
                for a in i{
                    black_box(unsafe { vec.get_unchecked_mut(a.taxi).bit_set(a.passenger) });

                }

                for (taxi,passengers) in vec.iter_mut().enumerate().filter(|(_,x)| x.bit_any()){
                    black_box(taxi);
                }

            }
        });
    }


}

pub mod atomic{
    use std::hint::black_box;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[bench]
    fn atomic_bool(b: &mut test::Bencher) {
        static value: AtomicBool = AtomicBool::new(false);
        b.iter(|| {
            black_box(for i in 0..10000 {
                let val = black_box(value.load(Ordering::SeqCst));
                if i == 9999{
                    return val;
                }
            });
            black_box(value.store(true,Ordering::SeqCst));
            value.load(Ordering::SeqCst)
        });
    }

    #[bench]
    fn static_bool(b: &mut test::Bencher) {
        static mut value: bool = false;
        b.iter(|| {
            black_box(for i in 0..10000 {
                let val = black_box(unsafe{ value });
                if i == 9999{
                    return val;
                }
            });
            black_box(unsafe{ value = true }) ;
            unsafe { value }
        });
    }
}