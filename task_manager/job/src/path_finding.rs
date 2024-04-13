use std::cmp::{min, Reverse};
use std::collections::BinaryHeap;
use bincode::config;
use serde::{Deserialize, Serialize};
use shared::task_manager::job_data::JobData;
use shared::task_manager::task::{Distance, split, TaskData, TaskResult};
use shared::utils::bit_set::BitSet;
use shared::utils::multilevel_vec_map::MultilevelVecMap;

pub fn dijkstra(adjacency_matrix: &Vec<Vec<u32>>, start: usize) -> Vec<u32> {
    let mut p_queue = BinaryHeap::from([Reverse((0, start))]);
    let mut visited = BitSet::with_capacity(adjacency_matrix.len());
    let mut distances = vec![u32::MAX; adjacency_matrix.len()];
    distances[start] = 0;

    loop{
        let Some(Reverse((current_distance, current_vertex))) = p_queue.pop() else {
            break;
        };

        if visited.contains(current_vertex){
            continue;
        }
        visited.insert(current_vertex);

        for (neighbor, &weight) in adjacency_matrix[current_vertex]
            .iter()
            .enumerate()
        {
            if weight != u32::MAX && !visited.contains(neighbor){
                let distance = current_distance + weight;

                if distance < distances[neighbor] {
                    distances[neighbor] = distance;
                    p_queue.push(Reverse((distance, neighbor)));
                }
            }
        }
    }

    return distances;
}

pub fn reduce_graph(mut job_data: JobData) -> JobData{
    let mut visited = BitSet::with_capacity(job_data.graph.len());

    for &(start,end) in &job_data.passengers{
        if !visited.contains(start) {
            let shortest_paths = dijkstra(&job_data.graph, start);
            job_data.graph[start] = shortest_paths;
            visited.insert(start);
        }
        if !visited.contains(end) {
            let shortest_paths = dijkstra(&job_data.graph, end);
            job_data.graph[end] = shortest_paths;
            visited.insert(end);
        }
    }

    for &vertex in &job_data.taxies{
        if !visited.contains(vertex) {
            let shortest_paths = dijkstra(&job_data.graph, vertex);
            job_data.graph[vertex] = shortest_paths;
            visited.insert(vertex);
        }
    }

    job_data
}

pub fn transform_graph(job_data: &JobData, taxi:usize, passengers: BitSet) -> Vec<Vec<u32>>{
    let origin_graph = &job_data.graph;
    let passengers: Vec<(usize,usize)> = passengers.iter().map(|x|job_data.passengers[x]).collect();
    let taxi = job_data.taxies[taxi];
    let len = passengers.len();
    let mut graph = vec![vec![u32::MAX;len + 1];len + 1];
    graph[0][0] = 0;
    for (&(i_start,i_end),i) in passengers.iter().zip(1..){
        graph[0][i] = origin_graph[taxi][i_start] + origin_graph[i_start][i_end];
        graph[i][0] = u32::MAX;

        for (&(j_start,j_end),j) in passengers.iter().zip(1..){
            graph[i][j] = origin_graph[i_end][j_start] + origin_graph[j_start][j_end];
        }
    }

    graph
}

pub fn min_path(graph: Vec<Vec<u32>>) -> Distance{
    let len = graph.len();
    let memo = vec![MultilevelVecMap::with_capacity(len,2);len];

    struct State{
        dist: Vec<Vec<u32>>,
        memo: Vec<MultilevelVecMap<Distance>>,
    }

    impl State{
        fn min_path(&mut self, i:usize, mut mask:BitSet<[u64;2]>) -> Distance{
            if mask.is_empty(){
                return 0;
            }

            if let Some(x) = self.memo[i].get(mask.storage()){
                return *x;

            }
            let mut res = Distance::MAX;
            let mut idx = 0;
            for j in mask.iter(){
                let path = self.min_path(j,mask.exclude(j)) + self.dist[i][j] as Distance;
                if res > path{
                    res = path;
                    idx = j;
                }
                //res = min(res,self.min_path(j,mask.exclude(j)) + self.dist[i][j])
            }

            self.memo[i].insert(mask.storage(),res);

            res
        }
    }

    let mut state = State{
        memo,
        dist: graph,
    };
    let min = state.min_path(0,BitSet::<[u64;2]>::init(true,len).exclude(0));

    min
}

pub fn job(job_data: Vec<u8>, task_data: Vec<u8>) -> Result<Vec<u8>, String>{

    let (job_data,_) = bincode::serde::decode_from_slice::<JobData,_>(&job_data, config::standard()).map_err(|err| err.to_string())?;
    let (task_data,_) = bincode::serde::decode_from_slice::<TaskData,_>(&task_data, config::standard()).map_err(|err| err.to_string())?;
    let job_data = reduce_graph(job_data);

    let result = task_data.combinations.map(|combination|{
        let passengers = BitSet::from(split(combination));
        let graph = transform_graph(&job_data, task_data.taxi, passengers);
        let min_path = min_path(graph);

        (combination,min_path)
    });

    let result = bincode::serde::encode_to_vec(TaskResult{
        taxi: task_data.taxi,
        result: result
    }, config::standard()).map_err(|err| err.to_string())?;

    Ok(result)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dijkstra() {
        let m = u32::MAX;
        let graph = vec![
            vec![m,m,5,m,m,m,m,m],
            vec![m,m,5,m,m,6,5,m],
            vec![5,5,m,m,3,9,9,m],
            vec![m,m,m,m,m,m,m,7],
            vec![m,m,3,m,m,m,m,1],
            vec![m,6,9,m,m,m,2,m],
            vec![m,5,9,m,m,2,m,2],
            vec![m,m,m,7,1,m,2,m],
        ];
        let start = 5;

        let result = dijkstra(&graph,start);

        println!("{:?}", result);
    }
    #[test]
    fn test_transform_graph() {
        let m = u32::MAX;
        let graph = vec![
            vec![m,m,5,m,m,m,m,m],
            vec![m,m,5,m,m,6,5,m],
            vec![5,5,m,m,3,9,9,m],
            vec![m,m,m,m,m,m,m,7],
            vec![m,m,3,m,m,m,m,1],
            vec![m,6,9,m,m,m,2,m],
            vec![m,5,9,m,m,2,m,2],
            vec![m,m,m,7,1,m,2,m],
        ];

        let mut job_data = JobData{
            graph,
            passengers: vec![(3,5),(6,4),(6,3),(7,4)],
            taxies: vec![2,1]
        };

        job_data = reduce_graph(job_data);

        println!("{:#?}", job_data);
        let graph = transform_graph(&job_data,0, BitSet::from_iter(vec![0,1,2,3]));
        println!("TRANSFORM");
        println!("{:#?}", graph);
    }

    #[test]
    fn test_min_distance() {
        let m = u32::MAX;
        let graph = vec![
            vec![m,m,5,m,m,m,m,m],
            vec![m,m,5,m,m,6,5,m],
            vec![5,5,m,m,3,9,9,m],
            vec![m,m,m,m,m,m,m,7],
            vec![m,m,3,m,m,m,m,1],
            vec![m,6,9,m,m,m,2,m],
            vec![m,5,9,m,m,2,m,2],
            vec![m,m,m,7,1,m,2,m],
        ];

        let mut job_data = JobData{
            graph,
            passengers: vec![(3,5),(6,5),(6,3),(7,1)],
            taxies: vec![2,1]
        };

        job_data = reduce_graph(job_data);

        println!("{:#?}", job_data);
        let graph = transform_graph(&job_data,0, BitSet::from_iter(vec![0,1,2,3]));
        println!("TRANSFORM");
        println!("{:#?}", graph);

        let min_path =min_path(graph);
        println!("{}", min_path);
    }
}
