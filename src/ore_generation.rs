//! Random ore veins for the stone produced by `--fillground`.
//!
//! Vanilla overworld height bands are mapped onto each column's underground fill
//! (`MIN_Y+1` .. `ground_y-3`) with vanilla Y=-64 at the column bottom and Y=64 at
//! the top of stone (sea level), so depth matches normal Minecraft worlds.

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
/// Vanilla bedrock floor, mapped to `underground_bottom`.
const VANILLA_ANCHOR_FLOOR: i32 = -64;
/// Vanilla sea level (top of normal stone), mapped to `underground_top`.
const VANILLA_ANCHOR_SURFACE: i32 = 64;

#[derive(Clone, Copy)]
enum HeightDistribution {
    /// Flat across the Y band.
    Uniform,
    /// Denser near the middle of the band (mean of two uniform samples).
    Triangle,
    /// Denser near the top of the band (max of two uniform samples).
    UpperTriangle,
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
        vanilla_y_max: 192,
        distribution: HeightDistribution::Triangle,
        size_min: 8,
        size_max: 17,
        spawn_tries: 14,
        air_exposure_skip: 0.55,
        rarity_one_in: None,
        rng_salt: 0xC0A1_0001,
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
        spawn_tries: 8,
        air_exposure_skip: 0.15,
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
        spawn_tries: 8,
        air_exposure_skip: 0.25,
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
        spawn_tries: 4,
        air_exposure_skip: 0.55,
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
        vanilla_y_max: 32,
        distribution: HeightDistribution::Uniform,
        size_min: 5,
        size_max: 8,
        spawn_tries: 6,
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
        spawn_tries: 4,
        air_exposure_skip: 0.0,
        rarity_one_in: None,
        rng_salt: 0x1A91_0001,
        min_height_above_base: None,
    },
    OrePlacement {
        block: LAPIS_ORE,
        deepslate_variant: DEEPSLATE_LAPIS_ORE,
        vanilla_y_min: -64,
        vanilla_y_max: 48,
        distribution: HeightDistribution::Uniform,
        size_min: 4,
        size_max: 4,
        spawn_tries: 3,
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
        vanilla_y_max: 48,
        distribution: HeightDistribution::Triangle,
        size_min: 5,
        size_max: 9,
        spawn_tries: 5,
        air_exposure_skip: 0.35,
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
        spawn_tries: 4,
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
        spawn_tries: 2,
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
        spawn_tries: 1,
        air_exposure_skip: 0.0,
        rarity_one_in: Some(14),
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
        spawn_tries: 1,
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

/// Vanilla Y at the top of the anchor range for this column (sea level, or higher when terrain is tall).
#[inline]
fn vanilla_anchor_ceiling(ground_y: i32) -> i32 {
    let Some((bottom, top)) = underground_bounds(ground_y) else {
        return VANILLA_ANCHOR_SURFACE;
    };
    let col_span = top - bottom;
    let sea_span = VANILLA_ANCHOR_SURFACE - VANILLA_ANCHOR_FLOOR;
    if col_span <= sea_span {
        VANILLA_ANCHOR_SURFACE
    } else {
        VANILLA_ANCHOR_FLOOR + col_span
    }
}

/// Map vanilla Y onto column Y (bedrock floor to anchor ceiling).
#[inline]
fn map_vanilla_y_to_column(
    vanilla_y: i32,
    underground_bottom: i32,
    underground_top: i32,
    vanilla_ceiling: i32,
) -> i32 {
    let col_span = (underground_top - underground_bottom).max(1);
    let clamped = vanilla_y.clamp(VANILLA_ANCHOR_FLOOR, vanilla_ceiling);
    let anchor_span = (vanilla_ceiling - VANILLA_ANCHOR_FLOOR).max(1) as f64;
    let t = (clamped - VANILLA_ANCHOR_FLOOR) as f64 / anchor_span;
    underground_bottom + (t * col_span as f64).round() as i32
}

fn clamped_vanilla_band(placement: &OrePlacement, ground_y: i32) -> Option<(i32, i32)> {
    let ceiling = vanilla_anchor_ceiling(ground_y);
    let lo = placement.vanilla_y_min.max(VANILLA_ANCHOR_FLOOR);
    let hi = placement.vanilla_y_max.min(ceiling);
    if lo > hi {
        return None;
    }
    Some((lo, hi))
}

fn placement_y_range(placement: &OrePlacement, ground_y: i32) -> Option<(i32, i32)> {
    let (underground_bottom, underground_top) = underground_bounds(ground_y)?;
    let (v_min, v_max) = clamped_vanilla_band(placement, ground_y)?;
    let vanilla_ceiling = vanilla_anchor_ceiling(ground_y);

    let mut y_min = map_vanilla_y_to_column(
        v_min,
        underground_bottom,
        underground_top,
        vanilla_ceiling,
    );
    let mut y_max = map_vanilla_y_to_column(
        v_max,
        underground_bottom,
        underground_top,
        vanilla_ceiling,
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
fn sample_vanilla_y(
    distribution: HeightDistribution,
    vanilla_y_min: i32,
    vanilla_y_max: i32,
    rng: &mut impl Rng,
) -> i32 {
    let (lo, hi) = if vanilla_y_min <= vanilla_y_max {
        (vanilla_y_min, vanilla_y_max)
    } else {
        (vanilla_y_max, vanilla_y_min)
    };
    match distribution {
        HeightDistribution::Uniform => rng.random_range(lo..=hi),
        HeightDistribution::Triangle => {
            let a = rng.random_range(lo..=hi);
            let b = rng.random_range(lo..=hi);
            (a + b) / 2
        }
        HeightDistribution::UpperTriangle => {
            let a = rng.random_range(lo..=hi);
            let b = rng.random_range(lo..=hi);
            a.max(b)
        }
    }
}

/// Sample a vein centre Y: distribution in vanilla space, then sea-level anchor map.
fn sample_placement_y(
    placement: &OrePlacement,
    ground_y: i32,
    rng: &mut impl Rng,
) -> Option<i32> {
    let (underground_bottom, underground_top) = underground_bounds(ground_y)?;
    let (v_min, v_max) = clamped_vanilla_band(placement, ground_y)?;
    let vanilla_y = sample_vanilla_y(placement.distribution, v_min, v_max, rng);
    let vanilla_ceiling = vanilla_anchor_ceiling(ground_y);
    let cy = map_vanilla_y_to_column(
        vanilla_y,
        underground_bottom,
        underground_top,
        vanilla_ceiling,
    );
    Some(cy.clamp(underground_bottom, underground_top))
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

                    if placement_y_range(placement, ground_y).is_none() {
                        continue;
                    }

                    let deepslate_top = deepslate_top_y(ground_y, args.ground_level);
                    let Some(cy) = sample_placement_y(placement, ground_y, &mut rng) else {
                        continue;
                    };
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
    const LAPIS_TRIANGLE: &OrePlacement = &ORE_PLACEMENTS[7];
    const GOLD_TRIANGLE: &OrePlacement = &ORE_PLACEMENTS[9];
    const DIAMOND_TRIANGLE: &OrePlacement = &ORE_PLACEMENTS[11];

    #[test]
    fn sea_level_anchor_maps_floor_and_surface() {
        let ground_y = 64;
        let (bottom, top) = underground_bounds(ground_y).unwrap();
        let ceiling = vanilla_anchor_ceiling(ground_y);
        assert_eq!(
            map_vanilla_y_to_column(VANILLA_ANCHOR_FLOOR, bottom, top, ceiling),
            bottom
        );
        assert_eq!(
            map_vanilla_y_to_column(VANILLA_ANCHOR_SURFACE, bottom, top, ceiling),
            top
        );
        assert_eq!(
            map_vanilla_y_to_column(384, bottom, top, ceiling),
            top,
            "above-sea-level vanilla Y clamps to surface stone top"
        );
    }

    #[test]
    fn tall_surface_spreads_ores_below_top() {
        let ground_y = 82;
        let top = ground_y - 3;
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let mut coal_sum = 0i64;
        let mut iron_sum = 0i64;
        const N: i32 = 3000;
        for _ in 0..N {
            coal_sum += sample_placement_y(COAL_MAIN, ground_y, &mut rng).unwrap() as i64;
        }
        let coal_mean = coal_sum as f64 / f64::from(N);
        assert!(
            coal_mean < f64::from(top) - 6.0,
            "coal mean {coal_mean} should stay below surface band at {top}"
        );
        assert!(
            clamped_vanilla_band(&ORE_PLACEMENTS[3], ground_y).is_none(),
            "mountain iron blob should not run when the column is shorter than vanilla Y=80"
        );
    }

    #[test]
    fn map_vanilla_endpoints_to_column() {
        let ground_y = 64;
        let ground_level = 0;
        let (coal_min, coal_max) = placement_y_range(COAL_MAIN, ground_y).unwrap();
        let (_, diamond_max) = placement_y_range(DIAMOND_TRIANGLE, ground_y).unwrap();
        let (_, gold_max) = placement_y_range(GOLD_TRIANGLE, ground_y).unwrap();
        let (_, lapis_max) = placement_y_range(LAPIS_TRIANGLE, ground_y).unwrap();
        let underground_top = ground_y - 3;
        let ds_top = deepslate_top_y(ground_y, ground_level);

        assert!(coal_min >= MIN_Y + 1);
        assert!(coal_max >= underground_top - 15, "coal should reach upper stone");
        assert!(coal_max <= underground_top);
        assert!(coal_max > diamond_max);
        assert!(
            diamond_max < underground_top - 25,
            "diamond stays well below surface stone"
        );
        assert!(gold_max > ds_top, "gold reaches stone above deepslate");
        assert!(lapis_max > ds_top, "lapis reaches stone above deepslate");
    }

    #[test]
    fn triangle_samples_near_vanilla_band_center() {
        let ground_y = 64;
        let (bottom, top) = underground_bounds(ground_y).unwrap();
        let ceiling = vanilla_anchor_ceiling(ground_y);
        let vanilla_mid = (DIAMOND_TRIANGLE.vanilla_y_min + DIAMOND_TRIANGLE.vanilla_y_max) / 2;
        let expected = map_vanilla_y_to_column(vanilla_mid, bottom, top, ceiling);
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let mut sum = 0i64;
        const N: i32 = 2000;
        for _ in 0..N {
            sum += sample_placement_y(DIAMOND_TRIANGLE, ground_y, &mut rng).unwrap() as i64;
        }
        let mean = sum as f64 / f64::from(N);
        assert!(
            (mean - f64::from(expected)).abs() < 8.0,
            "triangle mean {mean} should be near vanilla-centre map {expected}"
        );
    }

    #[test]
    fn copper_triangle_peaks_mid_column_not_bottom() {
        const COPPER: &OrePlacement = &ORE_PLACEMENTS[4];
        let ground_y = 64;
        let (bottom, top) = underground_bounds(ground_y).unwrap();
        let (v_lo, v_hi) = clamped_vanilla_band(COPPER, ground_y).unwrap();
        let vanilla_peak = (v_lo + v_hi) / 2;
        let ceiling = vanilla_anchor_ceiling(ground_y);
        let expected_peak = map_vanilla_y_to_column(vanilla_peak, bottom, top, ceiling);
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let mut sum = 0i64;
        const N: i32 = 2000;
        for _ in 0..N {
            sum += sample_placement_y(COPPER, ground_y, &mut rng).unwrap() as i64;
        }
        let mean = sum as f64 / f64::from(N);
        let column_mid = (bottom + top) as f64 / 2.0;
        assert!(
            mean > column_mid,
            "copper mean {mean} should sit above column mid {column_mid} (vanilla peak ~Y48)"
        );
        assert!(
            (mean - f64::from(expected_peak)).abs() < 10.0,
            "copper mean {mean} should be near mapped vanilla peak {expected_peak}"
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
        let (_coal_min, coal_max) = placement_y_range(COAL_MAIN, ground_y).unwrap();
        let (_, diamond_max) = placement_y_range(DIAMOND_TRIANGLE, ground_y).unwrap();
        let underground_top = ground_y - 3;
        assert!(coal_max > diamond_max);
        assert!(coal_max >= underground_top - 20);
        assert!(diamond_max < coal_max);
    }
}
