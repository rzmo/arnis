//! Random ore veins for the stone produced by `--fillground`.
//!
//! Vanilla overworld height bands are scaled onto each column's underground fill
//! (`MIN_Y+1` .. `ground_y-3`).

use crate::args::Args;
use crate::block_definitions::{
    Block, COAL_ORE, COPPER_ORE, DEEPSLATE, DEEPSLATE_COAL_ORE, DEEPSLATE_COPPER_ORE,
    DEEPSLATE_DIAMOND_ORE, DEEPSLATE_EMERALD_ORE, DEEPSLATE_GOLD_ORE, DEEPSLATE_IRON_ORE,
    DEEPSLATE_LAPIS_ORE, DEEPSLATE_REDSTONE_ORE, DIAMOND_ORE, EMERALD_ORE, GOLD_ORE, IRON_ORE,
    LAPIS_ORE, REDSTONE_ORE, STONE,
};
use crate::coordinate_system::cartesian::XZBBox;
use crate::deterministic_rng::coord_rng;
use crate::progress::emit_gui_progress_update;
use crate::world_editor::{WorldEditor, MIN_Y};
use colored::Colorize;
use rand::Rng;

const DEFAULT_GROUND_LEVEL: i32 = -62;
const MAX_DEEPSLATE_BAND: i32 = 64;
const ORE_SUBSTRATE: &[Block] = &[STONE, DEEPSLATE];
/// Vanilla Y used as the bottom of the mapping range.
const VANILLA_Y_FLOOR: i32 = -64;
/// Vanilla Y used as the top of the mapping range (iron high blob ends at 384).
const VANILLA_Y_CEILING: i32 = 384;

#[derive(Clone, Copy)]
enum HeightDistribution {
    /// Flat across the Y band.
    Uniform,
    /// Denser near the middle of the band (mean of two uniform samples).
    Triangle,
}

#[derive(Clone, Copy)]
struct OrePlacement {
    block: Block,
    deepslate_variant: Block,
    /// Shallowest vanilla Y for this placement.
    vanilla_y_min: i32,
    /// Deepest vanilla Y for this placement.
    vanilla_y_max: i32,
    distribution: HeightDistribution,
    size_min: u32,
    size_max: u32,
    /// Spawn tries per chunk for this placement.
    spawn_tries: u32,
    /// Chance to skip a vein when it would be air-exposed (0 = never skip).
    air_exposure_skip: f32,
    /// If Some(N), only run on 1 in N chunks.
    rarity_one_in: Option<u32>,
    rng_salt: u32,
    min_height_above_base: Option<i32>,
}

const ORE_PLACEMENTS: &[OrePlacement] = &[
    // Coal
    OrePlacement {
        block: COAL_ORE,
        deepslate_variant: DEEPSLATE_COAL_ORE,
        vanilla_y_min: 0,
        vanilla_y_max: 256,
        distribution: HeightDistribution::Triangle,
        size_min: 8,
        size_max: 17,
        spawn_tries: 20,
        air_exposure_skip: 0.35,
        rarity_one_in: None,
        rng_salt: 0xC0A1_0001,
        min_height_above_base: None,
    },
    OrePlacement {
        block: COAL_ORE,
        deepslate_variant: DEEPSLATE_COAL_ORE,
        vanilla_y_min: 48,
        vanilla_y_max: 320,
        distribution: HeightDistribution::Triangle,
        size_min: 6,
        size_max: 12,
        spawn_tries: 16,
        air_exposure_skip: 0.2,
        rarity_one_in: None,
        rng_salt: 0xC0A1_0002,
        min_height_above_base: None,
    },
    // Iron
    OrePlacement {
        block: IRON_ORE,
        deepslate_variant: DEEPSLATE_IRON_ORE,
        vanilla_y_min: -64,
        vanilla_y_max: 72,
        distribution: HeightDistribution::Uniform,
        size_min: 4,
        size_max: 4,
        spawn_tries: 10,
        air_exposure_skip: 0.0,
        rarity_one_in: None,
        rng_salt: 0x1E00_0001,
        min_height_above_base: None,
    },
    OrePlacement {
        block: IRON_ORE,
        deepslate_variant: DEEPSLATE_IRON_ORE,
        vanilla_y_min: -24,
        vanilla_y_max: 56,
        distribution: HeightDistribution::Triangle,
        size_min: 5,
        size_max: 9,
        spawn_tries: 10,
        air_exposure_skip: 0.0,
        rarity_one_in: None,
        rng_salt: 0x1E00_0002,
        min_height_above_base: None,
    },
    OrePlacement {
        block: IRON_ORE,
        deepslate_variant: DEEPSLATE_IRON_ORE,
        vanilla_y_min: 80,
        vanilla_y_max: 384,
        distribution: HeightDistribution::Triangle,
        size_min: 5,
        size_max: 9,
        spawn_tries: 10,
        air_exposure_skip: 0.0,
        rarity_one_in: None,
        rng_salt: 0x1E00_0003,
        min_height_above_base: None,
    },
    // Copper
    OrePlacement {
        block: COPPER_ORE,
        deepslate_variant: DEEPSLATE_COPPER_ORE,
        vanilla_y_min: -16,
        vanilla_y_max: 112,
        distribution: HeightDistribution::Triangle,
        size_min: 6,
        size_max: 10,
        spawn_tries: 16,
        air_exposure_skip: 0.0,
        rarity_one_in: None,
        rng_salt: 0xC0FF_0001,
        min_height_above_base: None,
    },
    // Redstone
    OrePlacement {
        block: REDSTONE_ORE,
        deepslate_variant: DEEPSLATE_REDSTONE_ORE,
        vanilla_y_min: -64,
        vanilla_y_max: 15,
        distribution: HeightDistribution::Uniform,
        size_min: 5,
        size_max: 8,
        spawn_tries: 4,
        air_exposure_skip: 0.0,
        rarity_one_in: None,
        rng_salt: 0xBED5_0001,
        min_height_above_base: None,
    },
    OrePlacement {
        block: REDSTONE_ORE,
        deepslate_variant: DEEPSLATE_REDSTONE_ORE,
        vanilla_y_min: -64,
        vanilla_y_max: -32,
        distribution: HeightDistribution::Triangle,
        size_min: 5,
        size_max: 8,
        spawn_tries: 4,
        air_exposure_skip: 0.0,
        rarity_one_in: None,
        rng_salt: 0xBED5_0002,
        min_height_above_base: None,
    },
    // Lapis
    OrePlacement {
        block: LAPIS_ORE,
        deepslate_variant: DEEPSLATE_LAPIS_ORE,
        vanilla_y_min: -32,
        vanilla_y_max: 32,
        distribution: HeightDistribution::Triangle,
        size_min: 4,
        size_max: 7,
        spawn_tries: 2,
        air_exposure_skip: 0.0,
        rarity_one_in: None,
        rng_salt: 0x1A91_0001,
        min_height_above_base: None,
    },
    OrePlacement {
        block: LAPIS_ORE,
        deepslate_variant: DEEPSLATE_LAPIS_ORE,
        vanilla_y_min: -64,
        vanilla_y_max: 64,
        distribution: HeightDistribution::Uniform,
        size_min: 4,
        size_max: 4,
        spawn_tries: 1,
        air_exposure_skip: 0.0,
        rarity_one_in: None,
        rng_salt: 0x1A91_0002,
        min_height_above_base: None,
    },
    // Gold
    OrePlacement {
        block: GOLD_ORE,
        deepslate_variant: DEEPSLATE_GOLD_ORE,
        vanilla_y_min: -64,
        vanilla_y_max: 32,
        distribution: HeightDistribution::Triangle,
        size_min: 5,
        size_max: 9,
        spawn_tries: 4,
        air_exposure_skip: 0.5,
        rarity_one_in: None,
        rng_salt: 0x60D0_0001,
        min_height_above_base: None,
    },
    OrePlacement {
        block: GOLD_ORE,
        deepslate_variant: DEEPSLATE_GOLD_ORE,
        vanilla_y_min: -64,
        vanilla_y_max: -48,
        distribution: HeightDistribution::Uniform,
        size_min: 5,
        size_max: 9,
        spawn_tries: 2,
        air_exposure_skip: 0.5,
        rarity_one_in: None,
        rng_salt: 0x60D0_0002,
        min_height_above_base: None,
    },
    // Diamond
    OrePlacement {
        block: DIAMOND_ORE,
        deepslate_variant: DEEPSLATE_DIAMOND_ORE,
        vanilla_y_min: -64,
        vanilla_y_max: 16,
        distribution: HeightDistribution::Triangle,
        size_min: 4,
        size_max: 4,
        spawn_tries: 7,
        air_exposure_skip: 0.5,
        rarity_one_in: None,
        rng_salt: 0xD1A1_0001,
        min_height_above_base: None,
    },
    OrePlacement {
        block: DIAMOND_ORE,
        deepslate_variant: DEEPSLATE_DIAMOND_ORE,
        vanilla_y_min: -64,
        vanilla_y_max: 16,
        distribution: HeightDistribution::Triangle,
        size_min: 5,
        size_max: 8,
        spawn_tries: 4,
        air_exposure_skip: 0.0,
        rarity_one_in: None,
        rng_salt: 0xD1A1_0002,
        min_height_above_base: None,
    },
    OrePlacement {
        block: DIAMOND_ORE,
        deepslate_variant: DEEPSLATE_DIAMOND_ORE,
        vanilla_y_min: -64,
        vanilla_y_max: 16,
        distribution: HeightDistribution::Triangle,
        size_min: 8,
        size_max: 12,
        spawn_tries: 2,
        air_exposure_skip: 0.0,
        rarity_one_in: Some(9),
        rng_salt: 0xD1A1_0003,
        min_height_above_base: None,
    },
    OrePlacement {
        block: DIAMOND_ORE,
        deepslate_variant: DEEPSLATE_DIAMOND_ORE,
        vanilla_y_min: -64,
        vanilla_y_max: -4,
        distribution: HeightDistribution::Uniform,
        size_min: 5,
        size_max: 8,
        spawn_tries: 2,
        air_exposure_skip: 0.5,
        rarity_one_in: None,
        rng_salt: 0xD1A1_0004,
        min_height_above_base: None,
    },
    // Emerald
    OrePlacement {
        block: EMERALD_ORE,
        deepslate_variant: DEEPSLATE_EMERALD_ORE,
        vanilla_y_min: -16,
        vanilla_y_max: 480,
        distribution: HeightDistribution::Triangle,
        size_min: 1,
        size_max: 3,
        spawn_tries: 12,
        air_exposure_skip: 0.0,
        rarity_one_in: None,
        rng_salt: 0xE1E1_0001,
        min_height_above_base: Some(80),
    },
];

#[inline]
fn column_deepslate_height(ground_y: i32, ground_level: i32) -> i32 {
    if ground_level <= DEFAULT_GROUND_LEVEL {
        return 0;
    }
    let underground_height = (ground_y - 3) - MIN_Y;
    (underground_height / 2).min(MAX_DEEPSLATE_BAND)
}

#[inline]
fn deepslate_top_y(ground_y: i32, ground_level: i32) -> i32 {
    let height = column_deepslate_height(ground_y, ground_level);
    if height == 0 {
        i32::MIN
    } else {
        MIN_Y + height
    }
}

#[inline]
fn underground_bounds(ground_y: i32) -> Option<(i32, i32)> {
    let underground_top = (ground_y - 3).max(MIN_Y + 1);
    let underground_bottom = MIN_Y + 1;
    if underground_top <= underground_bottom {
        return None;
    }
    Some((underground_bottom, underground_top))
}

/// Map vanilla Y onto column Y using a shared [vanilla_floor, vanilla_ceiling] range.
#[inline]
fn map_vanilla_y_to_column(
    vanilla_y: i32,
    vanilla_floor: i32,
    vanilla_ceiling: i32,
    underground_bottom: i32,
    underground_top: i32,
) -> i32 {
    let col_span = (underground_top - underground_bottom).max(1);
    let vanilla_span = (vanilla_ceiling - vanilla_floor).max(1);
    let clamped = vanilla_y.clamp(vanilla_floor, vanilla_ceiling);
    let t = (clamped - vanilla_floor) as f64 / vanilla_span as f64;
    underground_bottom + (t * col_span as f64).round() as i32
}

fn placement_y_range(placement: &OrePlacement, ground_y: i32) -> Option<(i32, i32)> {
    let (underground_bottom, underground_top) = underground_bounds(ground_y)?;

    let mut y_min = map_vanilla_y_to_column(
        placement.vanilla_y_min,
        VANILLA_Y_FLOOR,
        VANILLA_Y_CEILING,
        underground_bottom,
        underground_top,
    );
    let mut y_max = map_vanilla_y_to_column(
        placement.vanilla_y_max,
        VANILLA_Y_FLOOR,
        VANILLA_Y_CEILING,
        underground_bottom,
        underground_top,
    );
    if y_min > y_max {
        std::mem::swap(&mut y_min, &mut y_max);
    }
    y_min = y_min.clamp(underground_bottom, underground_top);
    y_max = y_max.clamp(underground_bottom, underground_top);
    if y_min > y_max {
        return None;
    }
    Some((y_min, y_max))
}

#[inline]
fn sample_y(
    distribution: HeightDistribution,
    y_min: i32,
    y_max: i32,
    rng: &mut impl Rng,
) -> i32 {
    match distribution {
        HeightDistribution::Uniform => rng.random_range(y_min..=y_max),
        HeightDistribution::Triangle => {
            let a = rng.random_range(y_min..=y_max);
            let b = rng.random_range(y_min..=y_max);
            ((a + b) / 2).clamp(y_min, y_max)
        }
    }
}

pub fn generate_ores(editor: &mut WorldEditor, xzbbox: &XZBBox, args: &Args) {
    println!("{} Sprinkling ore veins...", "[6b/7]".bold());
    emit_gui_progress_update(89.0, "Sprinkling ore veins...");

    let min_chunk_x = xzbbox.min_x() >> 4;
    let max_chunk_x = xzbbox.max_x() >> 4;
    let min_chunk_z = xzbbox.min_z() >> 4;
    let max_chunk_z = xzbbox.max_z() >> 4;

    for chunk_x in min_chunk_x..=max_chunk_x {
        for chunk_z in min_chunk_z..=max_chunk_z {
            for placement in ORE_PLACEMENTS {
                let mut rng = coord_rng(chunk_x, chunk_z, u64::from(placement.rng_salt));

                if let Some(one_in) = placement.rarity_one_in {
                    if one_in > 1 && rng.random_range(0..one_in) != 0 {
                        continue;
                    }
                }

                let max_attempts = placement.spawn_tries.saturating_mul(2);
                let n = rng.random_range(0..=max_attempts);
                for _ in 0..n {
                    if placement.air_exposure_skip > 0.0
                        && rng.random::<f32>() < placement.air_exposure_skip
                    {
                        continue;
                    }

                    let cx = (chunk_x << 4) + rng.random_range(0..16);
                    let cz = (chunk_z << 4) + rng.random_range(0..16);
                    let ground_y = editor.get_ground_level(cx, cz);

                    if let Some(min_above) = placement.min_height_above_base {
                        if ground_y - args.ground_level < min_above {
                            continue;
                        }
                    }

                    let Some((y_min, y_max)) = placement_y_range(placement, ground_y) else {
                        continue;
                    };

                    let deepslate_top = deepslate_top_y(ground_y, args.ground_level);
                    let cy = sample_y(placement.distribution, y_min, y_max, &mut rng);
                    let size = rng.random_range(placement.size_min..=placement.size_max);
                    place_vein(
                        editor,
                        placement,
                        deepslate_top,
                        cx,
                        cy,
                        cz,
                        size,
                        &mut rng,
                    );
                }
            }
        }
    }
}

fn place_vein(
    editor: &mut WorldEditor,
    placement: &OrePlacement,
    deepslate_top_y: i32,
    x: i32,
    y: i32,
    z: i32,
    size: u32,
    rng: &mut impl Rng,
) {
    let (mut cx, mut cy, mut cz) = (x, y, z);
    for _ in 0..size {
        if editor.check_for_block_absolute(cx, cy, cz, Some(ORE_SUBSTRATE), None) {
            let block = if deepslate_top_y > i32::MIN && cy < deepslate_top_y {
                placement.deepslate_variant
            } else {
                placement.block
            };
            editor.set_block_absolute(block, cx, cy, cz, Some(ORE_SUBSTRATE), None);
        }
        match rng.random_range(0..6) {
            0 => cx += 1,
            1 => cx -= 1,
            2 => cy += 1,
            3 => cy -= 1,
            4 => cz += 1,
            _ => cz -= 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    const COAL_MAIN: &OrePlacement = &ORE_PLACEMENTS[0];
    const COAL_SHALLOW: &OrePlacement = &ORE_PLACEMENTS[1];
    const DIAMOND_TRIANGLE: &OrePlacement = &ORE_PLACEMENTS[12];

    #[test]
    fn map_vanilla_endpoints_to_column() {
        let ground_y = 64;
        let (coal_min, coal_max) = placement_y_range(COAL_MAIN, ground_y).unwrap();
        let (diamond_min, diamond_max) = placement_y_range(DIAMOND_TRIANGLE, ground_y).unwrap();
        let underground_top = ground_y - 3;
        assert!(coal_min >= MIN_Y + 1);
        assert!(coal_max <= underground_top);
        assert!(coal_max > diamond_max);
        assert!(diamond_min <= coal_min);
        let (_, shallow_max) = placement_y_range(COAL_SHALLOW, ground_y).unwrap();
        assert!(
            shallow_max >= underground_top - 20,
            "shallow coal band should reach near the surface"
        );
    }

    #[test]
    fn triangle_samples_near_band_center() {
        let (y_min, y_max) = placement_y_range(DIAMOND_TRIANGLE, 64).unwrap();
        let mid = (y_min + y_max) / 2;
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let mut sum = 0i64;
        const N: i32 = 2000;
        for _ in 0..N {
            sum += sample_y(HeightDistribution::Triangle, y_min, y_max, &mut rng) as i64;
        }
        let mean = sum as f64 / f64::from(N);
        assert!(
            (mean - f64::from(mid)).abs() < 8.0,
            "triangle mean {mean} should be near mid {mid}"
        );
    }

    #[test]
    fn shallow_column_still_has_valid_band() {
        let ground_y = -55;
        let (y_min, y_max) = placement_y_range(DIAMOND_TRIANGLE, ground_y).unwrap();
        assert!(y_max - y_min >= 0);
        assert!(y_max <= ground_y - 3);
    }

    #[test]
    fn tall_column_spans_deep_and_shallow_blobs() {
        let ground_y = 200;
        let (coal_min, coal_max) = placement_y_range(COAL_MAIN, ground_y).unwrap();
        let (diamond_min, diamond_max) = placement_y_range(DIAMOND_TRIANGLE, ground_y).unwrap();
        assert!(coal_max > diamond_max);
        assert!(diamond_min < coal_min);
        assert!(coal_max < ground_y - 3);
    }
}
