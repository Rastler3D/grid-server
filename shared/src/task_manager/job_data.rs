use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use serde::{Deserialize, Deserializer, Serialize};
use serde::de::{DeserializeSeed, SeqAccess, Visitor};

#[derive(Serialize,Deserialize,Debug,Clone)]
pub struct JobData{
    pub graph: Vec<Vec<u32>>,
    pub taxies:Vec<usize>,
    pub passengers: Vec<(usize,usize)>
}

impl JobData {

    pub fn validate(&self) -> bool {
        let vertex_count = self.graph.len();
        for vertex in &self.graph{
            if vertex.len() != vertex_count{
                return false;
            }
        }

        for &position in &self.taxies {
            if position >= vertex_count{
                return false;
            }
        }

        for &(position,destination) in &self.passengers{
            if position >= vertex_count || destination >= vertex_count{
                return false;
            }
        }

        true
    }
}

