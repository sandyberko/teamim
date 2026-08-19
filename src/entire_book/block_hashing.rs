use std::{collections::HashMap, hint::black_box};

use blake3::Hash;
use fastcdc::v2020::FastCDC;
use similar::{Algorithm, TextDiff, utils::TextDiffRemapper};

use crate::{
    Cluster, PlacedCluster, PlacedDiac, diac::DiacMiss, place,
    tesseract_ext::bounding_box::BoundingBox,
};

const CHUNK_UNIT: usize = 32;
const MIN: usize = 4 * CHUNK_UNIT;
const AVG: usize = 16 * CHUNK_UNIT;
const MAX: usize = 64 * CHUNK_UNIT;

#[derive(Debug)]
struct Chunk {
    start: usize,
    end: usize,
    hash: Hash,
}

fn chunk(text: &str) -> Vec<Chunk> {
    FastCDC::new(text.as_bytes(), MIN, AVG, MAX)
        .map(|c| Chunk {
            start: c.offset,
            end: c.offset + c.length,
            hash: blake3::hash(&text.as_bytes()[c.offset..c.offset + c.length]),
        })
        .collect()
}

#[must_use]
pub fn diff_and_place(old: &str, new: &str, boxes: &[BoundingBox<char>]) -> Vec<PlacedCluster> {
    let old_chunks = chunk(old);
    let new_chunks = chunk(new);

    let mut index = HashMap::<Hash, usize>::new();
    for (i, c) in new_chunks.iter().enumerate() {
        index.insert(c.hash, i);
    }

    let mut old_pos = 0;
    let mut new_pos = 0;

    let mut rects = Vec::new();
    for (i, old_chunk) in old_chunks.iter().enumerate() {
        if let Some(&j) = index.get(&old_chunk.hash) {
            let new_chunk = &new_chunks[j];

            if old_pos < old_chunk.start || new_pos < new_chunk.start {
                let old = &old[old_pos..old_chunk.start];
                let new = &new[new_pos..new_chunk.start];

                let diff = TextDiff::configure().algorithm(Algorithm::Myers).diff_chars(old, new);
                let remapper = TextDiffRemapper::from_text_diff(&diff, old, new);
                let ops = diff.ops().iter().copied();

                let chunk_rects = place(boxes, old_pos / 2, new, &remapper, ops).unwrap();
                rects.extend(chunk_rects);
            }

            old_pos = old_chunk.end;
            new_pos = new_chunk.end;
        } else {
        }
    }

    if old_pos != old.len() || new_pos != new.len() {
        let diff = TextDiff::configure()
            .algorithm(Algorithm::Myers)
            .diff_chars(&old[old_pos..], &new[new_pos..]);
        let remapper = TextDiffRemapper::from_text_diff(&diff, old, new);
        let ops = diff.ops().iter().copied();
        let chunk_rects = place(boxes, old_pos / 2, new, &remapper, ops).unwrap();
        rects.extend(chunk_rects);
    }

    rects
}
