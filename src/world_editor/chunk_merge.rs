//! Decode / merge / re-encode Minecraft chunk sections for append-mode saves.

use super::common::{Blockstates, Chunk, PaletteItem, Section};
use fastnbt::{LongArray, Value};
use std::collections::HashMap;

const AIR: &str = "minecraft:air";

type Cell = (String, Option<Value>);

fn bits_per_block(palette_len: usize) -> usize {
    let mut bits = 4;
    while (1usize << bits) < palette_len {
        bits += 1;
    }
    bits
}

fn decode_section_cells(section: &Section) -> Vec<Cell> {
    let palette = &section.block_states.palette;
    if palette.is_empty() {
        return vec![(AIR.to_string(), None); 4096];
    }

    let mut cells = vec![(AIR.to_string(), None); 4096];

    if palette.len() == 1 {
        let name = palette[0].name.clone();
        let props = palette[0].properties.clone();
        if name != AIR {
            cells.fill((name, props));
        }
        return cells;
    }

    let Some(data) = &section.block_states.data else {
        let name = palette[0].name.clone();
        let props = palette[0].properties.clone();
        if name != AIR {
            cells.fill((name, props));
        }
        return cells;
    };

    let bits = bits_per_block(palette.len());
    let vals_per_long = 64 / bits;
    let mask = (1u64 << bits) - 1;

    for i in 0..4096usize {
        let long_idx = i / vals_per_long;
        let bit_offset = (i % vals_per_long) * bits;
        if long_idx < data.len() {
            let pi = ((data[long_idx] as u64 >> bit_offset) & mask) as usize;
            if pi < palette.len() {
                cells[i] = (palette[pi].name.clone(), palette[pi].properties.clone());
            }
        }
    }
    cells
}

fn encode_section_from_cells(cells: &[Cell], section_y: i8) -> Section {
    let mut unique: Vec<(String, Option<Value>)> = Vec::new();
    let mut lookup: HashMap<(String, Option<String>), usize> = HashMap::new();

    for cell in cells {
        let props_key = cell.1.as_ref().map(|p| format!("{p:?}"));
        let key = (cell.0.clone(), props_key);
        if let std::collections::hash_map::Entry::Vacant(e) = lookup.entry(key) {
            let idx = unique.len();
            e.insert(idx);
            unique.push((cell.0.clone(), cell.1.clone()));
        }
    }

    let bits = bits_per_block(unique.len());
    let mut data = vec![];
    let mut cur = 0i64;
    let mut cur_idx = 0usize;

    for (i, cell) in cells.iter().enumerate() {
        let props_key = cell.1.as_ref().map(|p| format!("{p:?}"));
        let p = lookup[&(cell.0.clone(), props_key)] as i64;

        if cur_idx + bits > 64 {
            data.push(cur);
            cur = 0;
            cur_idx = 0;
        }
        cur |= p << cur_idx;
        cur_idx += bits;
        if i == 4095 && cur_idx > 0 {
            data.push(cur);
        }
    }
    if cur_idx > 0 && (data.is_empty() || *data.last().unwrap() != cur) {
        data.push(cur);
    }

    let palette: Vec<PaletteItem> = unique
        .iter()
        .map(|(name, props)| PaletteItem {
            name: name.clone(),
            properties: props.clone(),
        })
        .collect();

    Section {
        block_states: Blockstates {
            palette,
            data: if unique.len() <= 1 {
                None
            } else {
                Some(LongArray::new(data))
            },
            other: Default::default(),
        },
        y: section_y,
        other: Default::default(),
    }
}

fn overlay_cells(base: &mut [Cell], overlay: &[Cell]) {
    for (i, (name, _)) in overlay.iter().enumerate() {
        if name != AIR {
            base[i] = overlay[i].clone();
        }
    }
}

/// Merge two chunk NBT blobs: `new` wins per non-air block; `existing` fills gaps.
pub fn merge_chunk_nbt(existing_bytes: &[u8], new_chunk: &Chunk) -> Result<Vec<u8>, String> {
    let existing: Chunk = fastnbt::from_bytes(existing_bytes)
        .map_err(|e| format!("Failed to parse existing chunk NBT: {e}"))?;

    let mut section_ys: std::collections::BTreeSet<i8> = std::collections::BTreeSet::new();
    for s in &existing.sections {
        section_ys.insert(s.y);
    }
    for s in &new_chunk.sections {
        section_ys.insert(s.y);
    }

    let mut merged_sections: Vec<Section> = Vec::new();
    for y in section_ys {
        let existing_sec = existing.sections.iter().find(|s| s.y == y);
        let new_sec = new_chunk.sections.iter().find(|s| s.y == y);

        match (existing_sec, new_sec) {
            (Some(e), Some(n)) => {
                let mut cells = decode_section_cells(e);
                overlay_cells(&mut cells, &decode_section_cells(n));
                merged_sections.push(encode_section_from_cells(&cells, y));
            }
            (Some(e), None) => merged_sections.push(e.clone()),
            (None, Some(n)) => merged_sections.push(n.clone()),
            (None, None) => {}
        }
    }

    let merged = Chunk {
        sections: merged_sections,
        x_pos: new_chunk.x_pos,
        z_pos: new_chunk.z_pos,
        is_light_on: new_chunk.is_light_on,
        other: {
            let mut other = existing.other.clone();
            for (k, v) in &new_chunk.other {
                other.insert(k.clone(), v.clone());
            }
            other
        },
    };

    let mut buf = Vec::with_capacity(8192);
    fastnbt::to_writer(&mut buf, &merged).map_err(|e| format!("Failed to serialize merged chunk: {e}"))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_definitions::STONE;
    use crate::world_editor::common::{ChunkToModify, SectionToModify};

    #[test]
    fn overlay_new_non_air_overwrites_existing() {
        let mut existing = ChunkToModify::default();
        existing.set_block(1, 64, 1, STONE);
        let existing_chunk = Chunk {
            sections: existing.sections().collect(),
            x_pos: 0,
            z_pos: 0,
            is_light_on: 0,
            other: Default::default(),
        };

        let mut new_mod = ChunkToModify::default();
        new_mod.set_block(1, 64, 1, crate::block_definitions::GRASS_BLOCK);
        let new_chunk = Chunk {
            sections: new_mod.sections().collect(),
            x_pos: 0,
            z_pos: 0,
            is_light_on: 0,
            other: Default::default(),
        };

        let existing_nbt = {
            let mut buf = Vec::new();
            fastnbt::to_writer(&mut buf, &existing_chunk).unwrap();
            buf
        };
        let merged_bytes = merge_chunk_nbt(&existing_nbt, &new_chunk).unwrap();
        let merged: Chunk = fastnbt::from_bytes(&merged_bytes).unwrap();
        assert!(!merged.sections.is_empty());
        let section = merged.sections.iter().find(|s| s.y == 4).expect("y=64 section");
        let cells = super::decode_section_cells(section);
        let idx = 1 + 16 * 1 + 256 * 0;
        assert_ne!(cells[idx].0, "minecraft:stone");
        assert_ne!(cells[idx].0, AIR);
    }

    #[test]
    fn overlay_preserves_existing_where_new_is_air() {
        let mut existing = ChunkToModify::default();
        existing.set_block(0, 64, 0, STONE);
        let existing_chunk = Chunk {
            sections: existing.sections().collect(),
            x_pos: 0,
            z_pos: 0,
            is_light_on: 0,
            other: Default::default(),
        };

        let new_chunk = Chunk {
            sections: vec![],
            x_pos: 0,
            z_pos: 0,
            is_light_on: 0,
            other: Default::default(),
        };

        let existing_nbt = {
            let mut buf = Vec::new();
            fastnbt::to_writer(&mut buf, &existing_chunk).unwrap();
            buf
        };

        let merged_bytes = merge_chunk_nbt(&existing_nbt, &new_chunk).unwrap();
        let merged: Chunk = fastnbt::from_bytes(&merged_bytes).unwrap();
        assert!(!merged.sections.is_empty());
    }
}
